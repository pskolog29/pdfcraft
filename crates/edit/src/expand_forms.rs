//! Expand Form XObjects that paint text into the page content stream so Edit text can see and
//! rewrite them. HTML→PDF converters (EvoPdf, etc.) wrap letter bodies in Forms; without this,
//! Edit selects the whole Form as grouped artwork and `text_lines` finds no paragraphs inside.

use pdfcraft_content::{Matrix, Op, parse, serialize_ops};
use pdfcraft_cos::{Dict, Document, ObjRef, Object};

use crate::text::{MAX_FORM_DEPTH, MAX_FORM_VISITS, form_call};
use crate::{EditError, text};

const RESOURCE_KEYS: &[&[u8]] = &[b"Font", b"XObject", b"ExtGState", b"ColorSpace", b"Pattern", b"Shading", b"Properties"];

/// Whether expanding Form XObjects on `page` would expose text that Edit currently cannot see.
pub fn needs_text_form_expand(doc: &Document, page: usize) -> Result<bool, EditError> {
    let p = text::page_dict(doc, page)?;
    let res = p.dict.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned()).unwrap_or_default();
    let mut path = Vec::new();
    let mut visits = 0;
    for (_, data) in text::content_streams(doc, &p.dict) {
        if stream_needs_expand(doc, &res, &data, &mut path, &mut visits) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Inline every text-bearing Form XObject drawn on `page` into the page's content streams and
/// merge its resources. Returns how many Form `Do` operators were expanded. Image-only Forms stay
/// as grouped artwork. Idempotent when there is nothing to expand.
pub fn expand_text_forms(doc: &mut Document, page: usize) -> Result<usize, EditError> {
    let p = text::page_dict(doc, page)?;
    let page_ref = p.obj;
    let mut res = p.dict.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned()).unwrap_or_default();
    let streams = text::content_streams(doc, &p.dict);
    if streams.is_empty() {
        return Ok(0);
    }
    let mut total = 0;
    let mut new_data = Vec::with_capacity(streams.len());
    let mut path = Vec::new();
    let mut visits = 0;
    let mut next_id = 0u32;
    for (_, data) in &streams {
        let ops = parse(data).ops;
        let (expanded, n) = expand_ops(doc, &ops, &mut res, &mut path, &mut visits, &mut next_id)?;
        total += n;
        new_data.push(if n == 0 { data.clone() } else { serialize_ops(&expanded) });
    }
    if total == 0 {
        return Ok(0);
    }
    // Drop any leftover clip setters on the page. Unexpanded sibling streams (EvoPdf often
    // adds a full-page `re W n` before a spacer) would still clip subsequent drawing; strip
    // W/W* everywhere so Edit ▸ move cannot hide inlined text.
    for data in &mut new_data {
        let ops = parse(data).ops;
        if ops.iter().any(|o| o.is("W") || o.is("W*")) {
            let cleaned: Vec<Op> = ops.into_iter().filter(|o| !o.is("W") && !o.is("W*")).collect();
            *data = serialize_ops(&cleaned);
        }
    }
    let contents = text::rewritten_contents(doc, &streams, new_data);
    doc.update_dict(page_ref, |d| {
        d.set(b"Contents".to_vec(), contents);
        d.set(b"Resources".to_vec(), Object::Dict(res));
    })?;
    Ok(total)
}

fn stream_needs_expand(doc: &Document, resources: &Dict, data: &[u8], path: &mut Vec<ObjRef>, visits: &mut usize) -> bool {
    for op in &parse(data).ops {
        if !op.is("Do") {
            continue;
        }
        let Some(name) = op.name(0) else { continue };
        if form_has_text(doc, resources, name, path, visits) {
            return true;
        }
    }
    false
}

fn form_has_text(doc: &Document, resources: &Dict, name: &[u8], path: &mut Vec<ObjRef>, visits: &mut usize) -> bool {
    if path.len() >= MAX_FORM_DEPTH || *visits >= MAX_FORM_VISITS {
        return false;
    }
    let Some(form) = form_call(doc, resources, name, Matrix::IDENTITY, path, visits) else {
        return false;
    };
    if let Some(r) = form.obj {
        path.push(r);
    }
    let ops = parse(&form.data).ops;
    let mut found = false;
    for op in &ops {
        match op.op.as_slice() {
            b"Tj" | b"TJ" | b"'" | b"\"" => {
                found = true;
                break;
            }
            b"Do" => {
                if let Some(n) = op.name(0)
                    && form_has_text(doc, &form.resources, n, path, visits)
                {
                    found = true;
                    break;
                }
            }
            _ => {}
        }
    }
    if form.obj.is_some() {
        path.pop();
    }
    found
}

fn data_has_text_ops(data: &[u8]) -> bool {
    parse(data).ops.iter().any(|op| matches!(op.op.as_slice(), b"Tj" | b"TJ" | b"'" | b"\""))
}

/// Drop a placement clip (`re W n` / `re W* n`) that immediately precedes an optional `cm`
/// before a Form `Do`. Leaving that clip after inlining traps moved text inside the old Form box.
fn drop_trailing_placement_clip(out: &mut Vec<Op>) {
    // Placement is often `re W n`, `re W n cm`, or `re W n q cm` immediately before Do.
    // Remove only the clip triplet; keep q/cm so q/Q balance stays intact.
    let mut end = out.len();
    if end == 0 {
        return;
    }
    if out[end - 1].is("cm") {
        end -= 1;
    }
    if end > 0 && out[end - 1].is("q") {
        end -= 1;
    }
    if end >= 3
        && out[end - 1].is("n")
        && (out[end - 2].is("W") || out[end - 2].is("W*"))
        && out[end - 3].is("re")
    {
        out.drain((end - 3)..end);
    }
}

/// Form producers (EvoPdf) start with a full-BBox `re W n` (sometimes twice). After inlining,
/// that clip still confines Ts/TJ moves used by Edit ▸ move.
fn strip_leading_re_clips(ops: Vec<Op>) -> Vec<Op> {
    let mut i = 0;
    while i + 2 < ops.len() && ops[i].is("re") && (ops[i + 1].is("W") || ops[i + 1].is("W*")) && ops[i + 2].is("n") {
        i += 3;
    }
    // Drop clip setters (W/W*) in the Form body. EvoPdf also clips via a path+W before the
    // white fill; keeping those clips makes Edit ▸ move hide the text outside the old box.
    ops[i..]
        .iter()
        .filter(|op| !op.is("W") && !op.is("W*"))
        .cloned()
        .collect()
}

fn expand_ops(
    doc: &Document,
    ops: &[Op],
    page_res: &mut Dict,
    path: &mut Vec<ObjRef>,
    visits: &mut usize,
    next_id: &mut u32,
) -> Result<(Vec<Op>, usize), EditError> {
    let mut out = Vec::with_capacity(ops.len());
    let mut expanded = 0;
    for op in ops {
        if op.is("Do")
            && let Some(name) = op.name(0)
            && let Some(form) = form_call(doc, page_res, name, Matrix::IDENTITY, path, visits)
        {
            if let Some(r) = form.obj {
                path.push(r);
            }
            let wants = data_has_text_ops(&form.data)
                || parse(&form.data).ops.iter().any(|o| {
                    o.is("Do") && o.name(0).is_some_and(|n| form_has_text(doc, &form.resources, n, path, visits))
                });
            if !wants {
                if form.obj.is_some() {
                    path.pop();
                }
                out.push(op.clone());
                continue;
            }
            let rename = merge_resources(page_res, &form.resources, next_id);
            // Keep clip ops through nested expand so drop_trailing_placement_clip can see `re W n`.
            let form_ops = remap_ops(&parse(&form.data).ops, &rename);
            let (inner, n) = expand_ops(doc, &form_ops, page_res, path, visits, next_id)?;
            let inner = strip_leading_re_clips(inner);
            expanded += 1 + n;
            // Placement clip around the old Do must not survive: it hides text after a move.
            drop_trailing_placement_clip(&mut out);
            out.push(Op::new("q", vec![]));
            if let Some(m) = form_matrix(doc, &form) {
                out.push(Op::new(
                    "cm",
                    vec![
                        pdfcraft_content::num(m[0]),
                        pdfcraft_content::num(m[1]),
                        pdfcraft_content::num(m[2]),
                        pdfcraft_content::num(m[3]),
                        pdfcraft_content::num(m[4]),
                        pdfcraft_content::num(m[5]),
                    ],
                ));
            }
            // Intentionally no Form BBox clip: edit moves use Ts/TJ and must remain visible.
            out.extend(inner);
            out.push(Op::new("Q", vec![]));
            if form.obj.is_some() {
                path.pop();
            }
            continue;
        }
        out.push(op.clone());
    }
    Ok((out, expanded))
}

fn form_matrix(doc: &Document, form: &crate::text::FormCall) -> Option<[f64; 6]> {
    // form_call already applied Matrix into form.ctm relative to IDENTITY caller; recover from the
    // form object's dictionary when present so we emit the Form's own Matrix at the inline site.
    let r = form.obj?;
    let Object::Stream(s) = &*doc.get(r) else { return None };
    let v = s.dict.get(b"Matrix").map(|m| doc.resolve(m))?;
    let a = v.as_array()?;
    let nums: Vec<f64> = a.iter().filter_map(|x| doc.resolve(x).as_f64()).collect();
    let m: [f64; 6] = nums.try_into().ok()?;
    if m == Matrix::IDENTITY.0 {
        None
    } else {
        Some(m)
    }
}


/// Merge `src` resource categories into `dst`. Conflicting names that point at different objects
/// are renamed; returns old→new name map for content remapping.
fn merge_resources(dst: &mut Dict, src: &Dict, next_id: &mut u32) -> std::collections::HashMap<Vec<u8>, Vec<u8>> {
    let mut rename = std::collections::HashMap::new();
    for &key in RESOURCE_KEYS {
        let Some(src_cat) = src.get(key).and_then(Object::as_dict) else { continue };
        let mut dst_cat = dst.get(key).and_then(Object::as_dict).cloned().unwrap_or_default();
        for (name, obj) in src_cat.iter() {
            match dst_cat.get(name) {
                Some(existing) if same_ref(existing, obj) => {}
                Some(_) => {
                    let fresh = unique_name(name, &dst_cat, next_id);
                    rename.insert(name.clone(), fresh.clone());
                    dst_cat.set(fresh, obj.clone());
                }
                None => {
                    dst_cat.set(name.clone(), obj.clone());
                }
            }
        }
        dst.set(key.to_vec(), Object::Dict(dst_cat));
    }
    rename
}

fn same_ref(a: &Object, b: &Object) -> bool {
    match (a.as_ref(), b.as_ref()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn unique_name(base: &[u8], cat: &Dict, next_id: &mut u32) -> Vec<u8> {
    loop {
        let candidate = format!("PCFm{}{}", next_id, String::from_utf8_lossy(base));
        *next_id = next_id.saturating_add(1);
        let bytes = candidate.into_bytes();
        if cat.get(&bytes).is_none() {
            return bytes;
        }
    }
}

fn remap_ops(ops: &[Op], rename: &std::collections::HashMap<Vec<u8>, Vec<u8>>) -> Vec<Op> {
    if rename.is_empty() {
        return ops.to_vec();
    }
    ops.iter()
        .map(|op| {
            let mut op = op.clone();
            // Resource names appear as the first name operand of Tf, Do, gs, CS/cs, SCN/scn (pattern), sh.
            if matches!(op.op.as_slice(), b"Tf" | b"Do" | b"gs" | b"CS" | b"cs" | b"SCN" | b"scn" | b"sh")
                && let Some(Object::Name(n)) = op.operands.first_mut()
                && let Some(new) = rename.get(n)
            {
                *n = new.clone();
            }
            op
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{reading_blocks, text_blocks};
    fn stream(dict: &str, body: &str) -> String {
        format!("<< {dict} /Length {} >>\nstream\n{body}\nendstream", body.len())
    }

    fn build(objs: &[&str]) -> Document {
        let mut body = Vec::new();
        let mut offsets = Vec::new();
        body.extend_from_slice(b"%PDF-1.4\n");
        for (i, o) in objs.iter().enumerate() {
            offsets.push(body.len());
            body.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            body.extend_from_slice(o.as_bytes());
            body.extend_from_slice(b"\nendobj\n");
        }
        let xref = body.len();
        body.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for off in offsets {
            body.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        body.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
        Document::open(std::sync::Arc::new(body)).expect("open")
    }

    #[test]
    fn expand_exposes_form_text_as_editable_paragraphs() {
        let page = stream("", "q 1 0 0 1 10 0 cm /Fm1 Do Q");
        let fm1 = stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 600 800] /Resources << /Font << /F1 5 0 R >> >>",
            "BT /F1 18 Tf 72 700 Td (Hello from a form) Tj ET",
        );
        let mut doc = build(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /XObject << /Fm1 4 0 R >> >> /Contents 6 0 R >>",
            &fm1,
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            &page,
        ]);
        assert!(text_blocks(&doc, 0).unwrap().is_empty());
        assert!(needs_text_form_expand(&doc, 0).unwrap());
        let n = expand_text_forms(&mut doc, 0).unwrap();
        assert_eq!(n, 1);
        let blocks = text_blocks(&doc, 0).unwrap();
        assert_eq!(blocks.len(), 1, "{blocks:?}");
        assert!(blocks[0].text.contains("Hello from a form"), "{:?}", blocks[0].text);
        assert!(!needs_text_form_expand(&doc, 0).unwrap());
        // Reading still agrees.
        assert_eq!(reading_blocks(&doc, 0).unwrap()[0].text, blocks[0].text);
    }

    #[test]
    fn expand_valilik_letter_yields_paragraph_blocks() {
        let path = "/workspace/arifpdf-build/pkg9ui/ip_erisim.pdf";
        let Ok(bytes) = std::fs::read(path) else { return };
        let mut doc = Document::open(std::sync::Arc::new(bytes)).expect("open");
        let before = text_blocks(&doc, 0).unwrap();
        let before_n = before.len();
        let forms_before = crate::page_images(&doc, 0).unwrap().iter().filter(|i| i.is_form).count();
        assert!(forms_before >= 1, "expected Form artwork before expand");
        assert!(needs_text_form_expand(&doc, 0).unwrap());
        let n = expand_text_forms(&mut doc, 0).unwrap();
        assert!(n >= 1, "expanded {n}");
        let blocks = text_blocks(&doc, 0).unwrap();
        assert!(
            blocks.len() > before_n,
            "expected more editable paragraphs after expand: before={before_n} after={} {:?}",
            blocks.len(),
            blocks.iter().map(|b| b.text.chars().take(40).collect::<String>()).collect::<Vec<_>>()
        );
        let joined: String = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join(" | ");
        assert!(
            joined.contains("VPN") || joined.contains("78.187") || joined.contains("erişim") || joined.contains("İlgi") || joined.contains("BİLGİ"),
            "{joined}"
        );
        let body = blocks
            .iter()
            .find(|b| b.text.contains("VPN") || b.text.contains("78.187") || b.text.contains("erişim") || b.text.contains("arz ederim"))
            .expect(&format!("body in {joined}"));
        assert!(body.rect[3] - body.rect[1] < 400.0, "body too tall: {:?} text={}", body.rect, body.text);
        let imgs = crate::page_images(&doc, 0).unwrap();
        let forms_after = imgs.iter().filter(|i| i.is_form).count();
        eprintln!("forms {forms_before}->{forms_after} images={} blocks={}", imgs.len(), blocks.len());
        for (i,b) in blocks.iter().enumerate() {
            eprintln!("block{i} h={:.1} {:?}", b.rect[3]-b.rect[1], b.text.chars().take(50).collect::<String>());
        }
        for (i,im) in imgs.iter().enumerate() {
            eprintln!("img{i} form={} rect={:?}", im.is_form, im.rect);
        }
        assert!(forms_after < forms_before, "text Forms should be gone: {forms_before} -> {forms_after}");
    }
    #[test]
    fn write_expanded_valilik_for_shot() {
        let path = "/workspace/arifpdf-build/pkg9ui/ip_erisim.pdf";
        let Ok(bytes) = std::fs::read(path) else { return };
        let mut doc = Document::open(std::sync::Arc::new(bytes)).expect("open");
        expand_text_forms(&mut doc, 0).unwrap();
        let out = pdfcraft_cos::write_full(&doc, &pdfcraft_cos::SaveOptions::default()).expect("save");
        std::fs::write("/workspace/arifpdf-build/pkg9ui/textedit/ip_erisim_expanded.pdf", &*out).unwrap();
    }

    #[test]
    fn dump_valilik_blocks_for_overlay() {
        let path = "/workspace/arifpdf-build/pkg9ui/ip_erisim.pdf";
        let Ok(bytes) = std::fs::read(path) else { return };
        let mut doc = Document::open(std::sync::Arc::new(bytes)).expect("open");
        expand_text_forms(&mut doc, 0).unwrap();
        let blocks = text_blocks(&doc, 0).unwrap();
        let mut out = String::from("595 842\n");
        for b in &blocks {
            let snip: String = b.text.chars().take(40).collect();
            out.push_str(&format!(
                "{:.2} {:.2} {:.2} {:.2}|{}\n",
                b.rect[0], b.rect[1], b.rect[2], b.rect[3], snip.replace("|", "/")
            ));
        }
        std::fs::write("/workspace/arifpdf-build/pkg9ui/textedit/blocks.txt", out).unwrap();
    }

    #[test]
    fn expand_then_move_valilik_blocks_stay_visible() {
        let path = "/workspace/arifpdf-build/pkg9ui/ip_erisim.pdf";
        let Ok(bytes) = std::fs::read(path) else { return };
        let mut doc = Document::open(std::sync::Arc::new(bytes)).expect("open");
        expand_text_forms(&mut doc, 0).unwrap();
        let before = text_blocks(&doc, 0).unwrap();
        assert!(before.len() >= 3, "need paragraphs: {}", before.len());
        // Find body / header / signature indexes
        let find = |pred: fn(&str) -> bool| {
            before.iter().position(|b| pred(&b.text)).expect("block")
        };
        let body = find(|t| t.contains("VPN") || t.contains("78.187") || t.contains("arz ederim"));
        let header = find(|t| t.contains("GÜMÜŞHANE") || t.contains("GUMUSHANE") || t.contains("VALİLİĞİ"));
        let sig = find(|t| t.contains("DOĞAN") || t.contains("Yardımcısı"));
        let body_text = before[body].text.clone();
        let header_text = before[header].text.clone();
        let sig_text = before[sig].text.clone();
        let body_rect = before[body].rect;
        // Move body down 40pt, header right 20, signature left 15
        crate::objects::move_objects(
            &mut doc,
            0,
            &[crate::objects::ObjectTarget { kind: crate::objects::ObjectKind::Text, index: body }],
            [0.0, -40.0],
        )
        .expect("move body");
        let after_body = text_blocks(&doc, 0).unwrap();
        let body2 = after_body.iter().find(|b| b.text.contains("VPN") || b.text.contains("78.187") || b.text == body_text || b.text.contains("arz ederim"));
        assert!(body2.is_some(), "body text vanished after move; blocks={:?}", after_body.iter().map(|b| b.text.chars().take(30).collect::<String>()).collect::<Vec<_>>());
        let body2 = body2.unwrap();
        assert!((body2.rect[1] - (body_rect[1] - 40.0)).abs() < 2.0, "body y expected ~{}, got {:?}", body_rect[1] - 40.0, body2.rect);
        // Re-find indexes after regroup
        let header_i = after_body.iter().position(|b| b.text == header_text || b.text.contains("VALİLİĞİ")).expect("header after body move");
        crate::objects::move_objects(
            &mut doc,
            0,
            &[crate::objects::ObjectTarget { kind: crate::objects::ObjectKind::Text, index: header_i }],
            [20.0, 0.0],
        )
        .expect("move header");
        let mid = text_blocks(&doc, 0).unwrap();
        assert!(mid.iter().any(|b| b.text.contains("VALİLİĞİ") || b.text == header_text), "header vanished");
        let sig_i = mid.iter().position(|b| b.text.contains("DOĞAN") || b.text == sig_text).expect("sig");
        crate::objects::move_objects(
            &mut doc,
            0,
            &[crate::objects::ObjectTarget { kind: crate::objects::ObjectKind::Text, index: sig_i }],
            [-15.0, 10.0],
        )
        .expect("move sig");
        let final_blocks = text_blocks(&doc, 0).unwrap();
        for (label, needle) in [("body", "arz"), ("header", "VALİ"), ("sig", "DOĞAN")] {
            assert!(
                final_blocks.iter().any(|b| b.text.contains(needle) || (label=="body" && (b.text.contains("VPN")||b.text.contains("78.187")))),
                "{label} missing after moves: {:?}",
                final_blocks.iter().map(|b| b.text.chars().take(25).collect::<String>()).collect::<Vec<_>>()
            );
        }
        // Edit a letter in the body paragraph
        let body_i = final_blocks
            .iter()
            .position(|b| b.text.contains("VPN") || b.text.contains("78.187") || b.text.contains("arz ederim"))
            .expect("body for edit");
        let mut edited = final_blocks[body_i].editor_text.clone();
        if let Some(i) = edited.find('a').or_else(|| edited.find('e')) {
            edited.replace_range(i..i+1, "X");
        } else {
            edited.push_str(" X");
        }
        crate::text::rewrite_block(&mut doc, 0, body_i, Some(&edited), &crate::text::BlockStyle::default()).expect("rewrite");
        let after_edit = text_blocks(&doc, 0).unwrap();
        assert!(after_edit.iter().any(|b| b.editor_text.contains('X') || b.text.contains('X')), "edit did not stick");
        // Save + reopen
        let bytes = pdfcraft_cos::write_full(&doc, &pdfcraft_cos::SaveOptions::default()).unwrap();
        let re = Document::open(std::sync::Arc::new(bytes)).unwrap();
        let re_blocks = text_blocks(&re, 0).unwrap();
        assert!(re_blocks.len() >= 3, "reopen lost paragraphs: {}", re_blocks.len());
        assert!(re_blocks.iter().any(|b| b.text.contains("VPN") || b.text.contains("78.187") || b.text.contains("arz") || b.text.contains('X')));
        assert!(re_blocks.iter().any(|b| b.text.contains("DOĞAN") || b.text.contains("Yardımcısı")));
        // Fonts still on page
        let page = crate::text::page_dict(&re, 0).unwrap();
        let res = page.dict.get(b"Resources").map(|r| re.resolve(r)).and_then(|r| r.as_dict().cloned()).unwrap_or_default();
        let fonts = res.get(b"Font").map(|r| re.resolve(r)).and_then(|r| r.as_dict().cloned()).unwrap_or_default();
        assert!(!fonts.is_empty(), "page lost fonts after expand+move+save");
        // No clip setters left in page content (placement/Form clips hide moved text).
        for (_, data) in crate::text::content_streams(&re, &page.dict) {
            for op in &pdfcraft_content::parse(&data).ops {
                assert!(
                    !op.is("W") && !op.is("W*"),
                    "page content still has clip op {:?} after expand+move",
                    String::from_utf8_lossy(&op.op)
                );
            }
        }
        // Far move: body shifted +120pt up — must remain findable (inventory) and we write for pdftoppm.
        let body_i = re_blocks
            .iter()
            .position(|b| b.text.contains("VPN") || b.text.contains("78.187") || b.text.contains("arz") || b.text.contains('X'))
            .expect("body after reopen");
        let mut doc2 = Document::open(std::sync::Arc::new(
            pdfcraft_cos::write_full(&re, &pdfcraft_cos::SaveOptions::default()).unwrap(),
        ))
        .unwrap();
        // re-expand not needed; already expanded
        let blocks2 = text_blocks(&doc2, 0).unwrap();
        let body_i = blocks2
            .iter()
            .position(|b| b.text.contains("VPN") || b.text.contains("78.187") || b.text.contains("arz") || b.text.contains('X'))
            .unwrap_or(body_i);
        let y0 = blocks2[body_i].rect[1];
        crate::objects::move_objects(
            &mut doc2,
            0,
            &[crate::objects::ObjectTarget { kind: crate::objects::ObjectKind::Text, index: body_i }],
            [0.0, 120.0],
        )
        .expect("far move");
        let blocks3 = text_blocks(&doc2, 0).unwrap();
        let body3 = blocks3
            .iter()
            .find(|b| b.text.contains("VPN") || b.text.contains("78.187") || b.text.contains("arz") || b.text.contains('X'))
            .expect("body vanished after far move");
        assert!((body3.rect[1] - (y0 + 120.0)).abs() < 3.0, "far move y: before={y0} after={:?}", body3.rect);
        let out = pdfcraft_cos::write_full(&doc2, &pdfcraft_cos::SaveOptions::default()).unwrap();
        std::fs::write("/workspace/arifpdf-build/pkg9ui/textedit/ip_erisim_moved.pdf", &*out).unwrap();
    }


    #[test]

    #[test]
    fn dump_expanded_stream_head() {
        let path = "/workspace/arifpdf-build/pkg9ui/ip_erisim.pdf";
        let Ok(bytes) = std::fs::read(path) else { return };
        let mut doc = Document::open(std::sync::Arc::new(bytes)).expect("open");
        expand_text_forms(&mut doc, 0).unwrap();
        let page = crate::text::page_dict(&doc, 0).unwrap();
        let streams = crate::text::content_streams(&doc, &page.dict);
        let mut out = String::new();
        for (i, (_, data)) in streams.iter().enumerate() {
            out.push_str(&format!("=== stream {i} len {} ===\n", data.len()));
            out.push_str(&String::from_utf8_lossy(&data[..data.len().min(2500)]));
            out.push_str("\n");
        }
        std::fs::write("/workspace/arifpdf-build/pkg9ui/textedit/expanded_streams.txt", out).unwrap();
    }

}
