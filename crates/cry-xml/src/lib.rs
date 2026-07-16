//! Shared XML representation for legacy CryEngine asset formats.
//!
//! Format-owning crates project typed fields from this tree while retaining
//! unrecognized attributes, elements, and text for forward-compatible tools.

use std::collections::BTreeMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlElement {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
    pub text: String,
    pub children: Vec<Self>,
}

impl XmlElement {
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Self> {
        self.children.iter().filter(move |child| child.name == name)
    }
}

/// Parse one CryEngine XML document into the shared element tree.
///
/// # Errors
///
/// Rejects malformed documents, unbalanced elements, empty documents, and
/// multiple document roots.
pub fn parse(xml: &str) -> Result<XmlElement, XmlError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<XmlElement>::new();
    let mut root = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => stack.push(element_from_start(&reader, &event)?),
            Ok(Event::Empty(event)) => {
                let element = element_from_start(&reader, &event)?;
                append_element(&mut stack, &mut root, element)?;
            }
            Ok(Event::Text(event)) => {
                if let Some(element) = stack.last_mut() {
                    element.text.push_str(
                        &event
                            .decode()
                            .map_err(|error| XmlError::Malformed(error.to_string()))?,
                    );
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(element) = stack.last_mut() {
                    element.text.push_str(
                        &event
                            .decode()
                            .map_err(|error| XmlError::Malformed(error.to_string()))?,
                    );
                }
            }
            Ok(Event::End(_)) => {
                let element = stack
                    .pop()
                    .ok_or_else(|| XmlError::Malformed("unexpected closing tag".to_owned()))?;
                append_element(&mut stack, &mut root, element)?;
            }
            Ok(Event::Eof) => break,
            Ok(Event::Decl(_) | Event::PI(_) | Event::Comment(_) | Event::DocType(_)) => {}
            Ok(Event::GeneralRef(_)) => {}
            Err(error) => return Err(XmlError::Malformed(error.to_string())),
        }
    }
    if !stack.is_empty() {
        return Err(XmlError::Malformed("unclosed XML element".to_owned()));
    }
    root.ok_or(XmlError::EmptyDocument)
}

fn element_from_start(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
) -> Result<XmlElement, XmlError> {
    let name = reader
        .decoder()
        .decode(event.name().as_ref())
        .map_err(|error| XmlError::Malformed(error.to_string()))?
        .into_owned();
    let mut attributes = BTreeMap::new();
    for attribute in event.attributes() {
        let attribute = attribute.map_err(|error| XmlError::Malformed(error.to_string()))?;
        let key = reader
            .decoder()
            .decode(attribute.key.as_ref())
            .map_err(|error| XmlError::Malformed(error.to_string()))?
            .into_owned();
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|error| XmlError::Malformed(error.to_string()))?
            .into_owned();
        attributes.insert(key, value);
    }
    Ok(XmlElement {
        name,
        attributes,
        text: String::new(),
        children: Vec::new(),
    })
}

fn append_element(
    stack: &mut [XmlElement],
    root: &mut Option<XmlElement>,
    element: XmlElement,
) -> Result<(), XmlError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(element);
        return Ok(());
    }
    if root.replace(element).is_some() {
        return Err(XmlError::MultipleRoots);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum XmlError {
    #[error("malformed CryEngine XML: {0}")]
    Malformed(String),
    #[error("CryEngine XML document is empty")]
    EmptyDocument,
    #[error("CryEngine XML document has multiple roots")]
    MultipleRoots,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_unknown_attributes_text_and_children() {
        let root = parse(r#"<Root custom="&amp;"><Child x="1"> value </Child></Root>"#).unwrap();
        assert_eq!(root.attributes["custom"], "&");
        assert_eq!(root.children[0].attributes["x"], "1");
        assert_eq!(root.children[0].text, " value ");
    }
}
