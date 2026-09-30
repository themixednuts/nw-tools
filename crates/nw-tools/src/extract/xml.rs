//! Complete extract for leftover pak XML (ADR-0001).

use crate::grep::adapter::{FactDraft, FactKind};

pub fn leftover_xml_facts(bytes: &[u8]) -> Result<Vec<FactDraft>, String> {
    match walk_events(bytes) {
        Ok(facts) if !facts.is_empty() => Ok(facts),
        Ok(_) | Err(_) => {
            let recovered = recover(bytes);
            if recovered.is_empty() {
                Err("undecodable".to_owned())
            } else {
                Ok(recovered)
            }
        }
    }
}

fn walk_events(bytes: &[u8]) -> Result<Vec<FactDraft>, quick_xml::Error> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let config = reader.config_mut();
    config.trim_text(true);

    let mut facts = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            quick_xml::events::Event::Start(tag) => {
                let name = qname_text(tag.name());
                if name.is_empty() {
                    buf.clear();
                    continue;
                }
                stack.push(name.clone());
                emit_element(&mut facts, &stack, &name);
                emit_attrs(&mut facts, &stack, &tag);
            }
            quick_xml::events::Event::Empty(tag) => {
                let name = qname_text(tag.name());
                if name.is_empty() {
                    buf.clear();
                    continue;
                }
                stack.push(name.clone());
                emit_element(&mut facts, &stack, &name);
                emit_attrs(&mut facts, &stack, &tag);
                stack.pop();
            }
            quick_xml::events::Event::End(_) => {
                stack.pop();
            }
            quick_xml::events::Event::Text(text) => {
                if let Ok(decoded) = text.xml11_content() {
                    push_value(&mut facts, path(&stack), decoded.as_ref());
                }
            }
            quick_xml::events::Event::CData(text) => {
                if let Ok(decoded) = text.decode() {
                    push_value(&mut facts, path(&stack), decoded.as_ref());
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(facts)
}

fn emit_element(facts: &mut Vec<FactDraft>, stack: &[String], name: &str) {
    push_value(facts, path(stack), name);
}

fn emit_attrs(
    facts: &mut Vec<FactDraft>,
    stack: &[String],
    tag: &quick_xml::events::BytesStart<'_>,
) {
    for attr in tag.attributes().with_checks(false) {
        let Ok(attr) = attr else {
            continue;
        };
        let key = qname_text(attr.key);
        if key.is_empty() {
            continue;
        }
        let parent = path(stack);
        push_value(facts, parent.clone(), &key);
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map(|value| value.into_owned())
            .unwrap_or_else(|_| String::from_utf8_lossy(&attr.value).into_owned());
        push_value(facts, format!("{parent}/@{key}"), &value);
    }
}

fn qname_text(name: quick_xml::name::QName<'_>) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).into_owned()
}

fn path(stack: &[String]) -> String {
    stack.join("/")
}

fn push_value(facts: &mut Vec<FactDraft>, field: String, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    facts.push(FactDraft {
        field,
        value: value.to_owned(),
        kind: FactKind::Xml,
        loc_a: 0,
        loc_b: 0,
    });
}

fn recover(bytes: &[u8]) -> Vec<FactDraft> {
    let text = String::from_utf8_lossy(bytes);
    let mut facts = Vec::new();
    recover_quoted(&text, &mut facts);
    recover_tags_and_text(&text, &mut facts);
    facts
}

fn recover_quoted(text: &str, facts: &mut Vec<FactDraft>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let quote = bytes[index];
        if quote != b'"' && quote != b'\'' {
            index += 1;
            continue;
        }
        let Some(end) = bytes[index + 1..]
            .iter()
            .position(|&byte| byte == quote)
            .map(|rel| index + 1 + rel)
        else {
            break;
        };
        let value = &text[index + 1..end];
        push_value(facts, String::new(), value);
        index = end + 1;
    }
}

fn recover_tags_and_text(text: &str, facts: &mut Vec<FactDraft>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        let start = index + 1;
        if start < bytes.len()
            && (bytes[start] == b'/' || bytes[start] == b'!' || bytes[start] == b'?')
        {
            index = start;
            continue;
        }
        let end = bytes[start..]
            .iter()
            .position(|&byte| {
                byte == b'>' || byte == b' ' || byte == b'\t' || byte == b'/' || byte == b'\n'
            })
            .map_or(bytes.len(), |rel| start + rel);
        if end > start {
            push_value(facts, String::new(), &text[start..end]);
        }
        if let Some(rel) = bytes[end..].iter().position(|&byte| byte == b'>') {
            let close = end + rel;
            if close + 1 < bytes.len() {
                if let Some(next) = bytes[close + 1..].iter().position(|&byte| byte == b'<') {
                    push_value(facts, String::new(), &text[close + 1..close + 1 + next]);
                }
            }
            index = close + 1;
        } else {
            break;
        }
    }
}
