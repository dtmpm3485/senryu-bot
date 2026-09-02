mod backup;
mod bot;
mod commands;
mod config;
mod crypto;
mod db;
mod detector;
mod health;
mod metrics;
mod models;
mod state;

use pyo3::{exceptions::PyRuntimeError, prelude::*};

/// Start the Rust Discord senryu bot. This call blocks until the bot stops.
#[pyfunction]
fn run(py: Python<'_>, token: String) -> PyResult<()> {
    if token.trim().is_empty() {
        return Err(PyRuntimeError::new_err("Discord Bot token is empty"));
    }
    let result = py.detach(move || bot::run_blocking(token).map_err(|e| format!("{e:#}")));
    result.map_err(PyRuntimeError::new_err)
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)?;
    Ok(())
}
