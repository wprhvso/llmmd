use pyo3::{
    prelude::*,
    types::{PyDict, PyList},
};

pub mod from_markdown;
pub mod limits;
pub mod to_telegram;

use crate::to_telegram::process_llm_markdown_sync;

#[pyfunction]
#[pyo3(signature = (markdown, with_photo=false))]
fn process_markdown<'py>(
    py: Python<'py>,
    markdown: &str,
    with_photo: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let result = process_llm_markdown_sync(markdown, with_photo);

    let py_list = PyList::empty(py);

    for (text, entities) in result {
        let chunk_dict = PyDict::new(py);
        chunk_dict.set_item("text", text)?;

        let py_entities = PyList::empty(py);
        for entity in entities {
            let ent_dict = PyDict::new(py);
            ent_dict.set_item("type", entity.kind.as_str())?;
            ent_dict.set_item("offset", entity.offset)?;
            ent_dict.set_item("length", entity.length)?;
            ent_dict.set_item("url", entity.url)?;
            ent_dict.set_item("language", entity.language)?;
            py_entities.append(ent_dict)?;
        }

        chunk_dict.set_item("entities", py_entities)?;
        py_list.append(chunk_dict)?;
    }

    Ok(py_list.into_any())
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
