//! Python API Bindings via C FFI.
//!
//! Provides a C-compatible foreign function interface that Python can access
//! via `ctypes`. This avoids the need for pyo3 compilation while still
//! allowing full programmatic access to OpenStruct's structural analysis.

use crate::analysis::Analysis;
use crate::analysis::AnalysisResults as RustAnalysisResults;
use crate::elements::Element;
use crate::materials::Material;
use crate::model::Model;
use crate::sections::Section;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int};
use std::ptr;

/// Opaque handle to a Model
pub type ModelHandle = *mut Model;

// Thread-local storage for analysis results
thread_local! {
    static RESULTS: RefCell<HashMap<usize, RustAnalysisResults>> = RefCell::new(HashMap::new());
}

// ============================================================================
// Model Management
// ============================================================================

#[no_mangle]
pub extern "C" fn openstruct_model_new(name: *const c_char) -> ModelHandle {
    let name_str = if name.is_null() {
        "Untitled".to_string()
    } else {
        unsafe { CStr::from_ptr(name) }
            .to_str()
            .unwrap_or("Untitled")
            .to_string()
    };
    let model = Box::new(Model::new(&name_str));
    Box::into_raw(model)
}

#[no_mangle]
pub extern "C" fn openstruct_model_free(model: ModelHandle) {
    if !model.is_null() {
        let addr = model as usize;
        RESULTS.with(|r| r.borrow_mut().remove(&addr));
        unsafe {
            let _ = Box::from_raw(model);
        }
    }
}

#[no_mangle]
pub extern "C" fn openstruct_model_name(model: ModelHandle) -> *mut c_char {
    if model.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let m = &*model;
        CString::new(m.name.clone()).unwrap().into_raw()
    }
}

// ============================================================================
// Node Management
// ============================================================================

#[no_mangle]
pub extern "C" fn openstruct_add_node(
    model: ModelHandle,
    x: c_double,
    y: c_double,
    z: c_double,
) -> c_int {
    if model.is_null() {
        return -1;
    }
    unsafe {
        let m = &mut *model;
        let id = m.add_node(x, y, z);
        id as c_int
    }
}

#[no_mangle]
pub extern "C" fn openstruct_set_restraint(
    model: ModelHandle,
    node_id: c_int,
    ux: c_int,
    uy: c_int,
    uz: c_int,
    rx: c_int,
    ry: c_int,
    rz: c_int,
) -> c_int {
    if model.is_null() {
        return -1;
    }
    unsafe {
        let m = &mut *model;
        if let Some(node) = m.nodes.get_mut(&(node_id as usize)) {
            node.restrained = [
                ux != 0,
                uy != 0,
                uz != 0,
                rx != 0,
                ry != 0,
                rz != 0,
            ];
            0
        } else {
            -1
        }
    }
}

// ============================================================================
// Material & Section Management
// ============================================================================

#[no_mangle]
pub extern "C" fn openstruct_add_material_steel(model: ModelHandle) -> c_int {
    if model.is_null() {
        return -1;
    }
    unsafe {
        let m = &mut *model;
        let id = m.add_material(Material::steel());
        id as c_int
    }
}

#[no_mangle]
pub extern "C" fn openstruct_add_material_concrete(
    model: ModelHandle,
    _fc: c_double,
) -> c_int {
    if model.is_null() {
        return -1;
    }
    unsafe {
        let m = &mut *model;
        let id = m.add_material(Material::concrete());
        id as c_int
    }
}

#[no_mangle]
pub extern "C" fn openstruct_add_section_rectangular(
    model: ModelHandle,
    b: c_double,
    h: c_double,
) -> c_int {
    if model.is_null() {
        return -1;
    }
    unsafe {
        let m = &mut *model;
        let id = m.add_section(Section::rectangular("RECT", b, h));
        id as c_int
    }
}

// ============================================================================
// Element Management
// ============================================================================

#[no_mangle]
pub extern "C" fn openstruct_add_frame(
    model: ModelHandle,
    node_i: c_int,
    node_j: c_int,
    material_id: c_int,
    section_id: c_int,
) -> c_int {
    if model.is_null() {
        return -1;
    }
    unsafe {
        let m = &mut *model;
        let elem = Element::frame(
            0,
            node_i as usize,
            node_j as usize,
            material_id as usize,
            section_id as usize,
        );
        let id = m.add_element(elem);
        id as c_int
    }
}

// ============================================================================
// Loads
// ============================================================================

#[no_mangle]
pub extern "C" fn openstruct_add_nodal_load(
    model: ModelHandle,
    node_id: c_int,
    fx: c_double,
    fy: c_double,
    fz: c_double,
    mx: c_double,
    my: c_double,
    mz: c_double,
) -> c_int {
    if model.is_null() {
        return -1;
    }
    unsafe {
        let m = &mut *model;
        // Find or create a load case
        if m.load_cases.is_empty() {
            m.load_cases.push(crate::loads::LoadCase {
                id: 1,
                name: "DEAD".to_string(),
                pattern: crate::loads::LoadType::Dead,
                self_weight_multiplier: 0.0,
                nodal_loads: Vec::new(),
                distributed_loads: Vec::new(),
                p_delta: false,
            });
        }
        if let Some(case) = m.load_cases.first_mut() {
            case.nodal_loads.push(crate::loads::NodalLoad {
                node_id: node_id as usize,
                fx,
                fy,
                fz,
                mx,
                my,
                mz,
            });
        }
        0
    }
}

// ============================================================================
// Analysis
// ============================================================================

#[no_mangle]
pub extern "C" fn openstruct_run_analysis(model: ModelHandle) -> c_int {
    if model.is_null() {
        return -1;
    }
    unsafe {
        let m = &*model;
        let analysis = Analysis::new(m.clone());
        let results = analysis.run_static();
        let addr = model as usize;
        RESULTS.with(|r| r.borrow_mut().insert(addr, results));
        0
    }
}

// ============================================================================
// Results Retrieval
// ============================================================================

/// Helper to find node index in results
fn node_index(model: ModelHandle, node_id: usize) -> Option<usize> {
    if model.is_null() {
        return None;
    }
    let addr = model as usize;
    RESULTS.with(|r| {
        let results = r.borrow();
        let res = results.get(&addr)?;
        res.node_ids.iter().position(|&id| id == node_id)
    })
}

#[no_mangle]
pub extern "C" fn openstruct_get_displacement(
    model: ModelHandle,
    node_id: c_int,
    component: c_int,
) -> c_double {
    if model.is_null() || component < 0 || component > 5 {
        return 0.0;
    }
    let addr = model as usize;
    let nid = node_id as usize;
    let comp = component as usize;
    RESULTS.with(|r| {
        let results = r.borrow();
        if let Some(res) = results.get(&addr) {
            if let Some(idx) = res.node_ids.iter().position(|&id| id == nid) {
                let flat_idx = idx * 6 + comp;
                if flat_idx < res.displacements.len() {
                    return res.displacements[flat_idx];
                }
            }
        }
        0.0
    })
}

#[no_mangle]
pub extern "C" fn openstruct_get_reaction(
    model: ModelHandle,
    node_id: c_int,
    component: c_int,
) -> c_double {
    if model.is_null() || component < 0 || component > 5 {
        return 0.0;
    }
    let addr = model as usize;
    let nid = node_id as usize;
    let comp = component as usize;
    RESULTS.with(|r| {
        let results = r.borrow();
        if let Some(res) = results.get(&addr) {
            if let Some(idx) = res.node_ids.iter().position(|&id| id == nid) {
                let flat_idx = idx * 6 + comp;
                if flat_idx < res.reactions.len() {
                    return res.reactions[flat_idx];
                }
            }
        }
        0.0
    })
}

#[no_mangle]
pub extern "C" fn openstruct_get_member_force(
    model: ModelHandle,
    element_id: c_int,
    component: c_int,
) -> c_double {
    if model.is_null() || component < 0 || component > 11 {
        return 0.0;
    }
    let addr = model as usize;
    let eid = element_id as usize;
    let comp = component as usize;
    RESULTS.with(|r| {
        let results = r.borrow();
        if let Some(res) = results.get(&addr) {
            if let Some(idx) = res.element_ids.iter().position(|&id| id == eid) {
                let flat_idx = idx * 12 + comp;
                if flat_idx < res.member_forces.len() {
                    return res.member_forces[flat_idx];
                }
            }
        }
        0.0
    })
}

// ============================================================================
// Utility
// ============================================================================

#[no_mangle]
pub extern "C" fn openstruct_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

#[no_mangle]
pub extern "C" fn openstruct_version() -> *mut c_char {
    CString::new("0.1.0").unwrap().into_raw()
}

// ============================================================================
// Python Wrapper
// ============================================================================

/// The ctypes wrapper shipped with the package, embedded so it can be written
/// out by the Rust API and checked by the tests without touching the filesystem.
pub const PYTHON_WRAPPER: &str = include_str!("../python/openstruct/__init__.py");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_wrapper_not_empty() {
        assert!(!PYTHON_WRAPPER.is_empty());
        assert!(PYTHON_WRAPPER.contains("class Model"));
        assert!(PYTHON_WRAPPER.contains("def add_node"));
        assert!(PYTHON_WRAPPER.contains("def run_analysis"));
    }
}
