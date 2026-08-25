use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};

pub mod from_markdown;
pub mod limits;
pub mod to_telegram;

use crate::to_telegram::{MessageChunk, MessageEntity};

impl<'py> IntoPyObject<'py> for MessageEntity {
    type Error = PyErr;
    type Output = Bound<'py, PyDict>;
    type Target = PyDict;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let entity = PyDict::new(py);
        entity.set_item("type", self.kind.as_str())?;
        entity.set_item("offset", self.offset)?;
        entity.set_item("length", self.length)?;
        entity.set_item("url", self.url)?;
        entity.set_item("language", self.language)?;
        Ok(entity)
    }
}

impl<'py> IntoPyObject<'py> for MessageChunk {
    type Error = PyErr;
    type Output = Bound<'py, PyDict>;
    type Target = PyDict;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let chunk = PyDict::new(py);
        chunk.set_item("text", self.text)?;
        chunk.set_item("entities", self.entities)?;
        Ok(chunk)
    }
}

#[pyfunction]
#[pyo3(signature = (markdown, with_photo=false))]
fn process_markdown<'py>(
    py: Python<'py>,
    markdown: &str,
    with_photo: bool,
) -> PyResult<Bound<'py, PyList>> {
    let owned = markdown.to_owned();
    let chunks = py.detach(move || to_telegram::process_markdown(&owned, with_photo));
    PyList::new(py, chunks)
}

#[pymodule]
#[pyo3(name = "_native")]
fn native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(process_markdown, m)?)?;
    m.add("MESSAGE_LIMIT", crate::limits::MESSAGE_LIMIT)?;
    m.add("CAPTION_LIMIT", crate::limits::CAPTION_LIMIT)?;
    m.add("MAX_ENTITIES", crate::limits::MAX_ENTITIES)?;
    Ok(())
}
