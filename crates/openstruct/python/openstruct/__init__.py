"""OpenStrut Python bindings.

This package provides Python access to OpenStrut's structural analysis
via a C FFI shared library loaded with ctypes.

Build the shared library first:
    cargo build --release -p openstrut --features ffi

Then install this package:
    pip install -e .
"""

# The wrapper code is embedded in the Rust binary as PYTHON_WRAPPER.
# This file provides the ctypes-based interface.

import ctypes
import os
import sys

_lib_paths = [
    os.path.join(os.path.dirname(__file__), "libopenstrut.so"),
    os.path.join(os.path.dirname(__file__), "libopenstrut.dylib"),
    os.path.join(os.path.dirname(__file__), "openstrut.dll"),
    os.path.join(os.path.dirname(__file__), "..", "target", "release", "libopenstrut.so"),
    os.path.join(os.path.dirname(__file__), "..", "target", "release", "libopenstrut.dylib"),
    os.path.join(os.path.dirname(__file__), "..", "target", "release", "openstrut.dll"),
]

_lib = None
for path in _lib_paths:
    try:
        _lib = ctypes.CDLL(path)
        break
    except OSError:
        continue

if _lib is None:
    raise ImportError(
        "Could not find OpenStrut shared library. "
        "Build with: cargo build --release -p openstrut"
    )

_lib.openstrut_model_new.restype = ctypes.c_void_p
_lib.openstrut_model_new.argtypes = [ctypes.c_char_p]
_lib.openstrut_model_free.restype = None
_lib.openstrut_model_free.argtypes = [ctypes.c_void_p]
_lib.openstrut_add_node.restype = ctypes.c_int
_lib.openstrut_add_node.argtypes = [ctypes.c_void_p, ctypes.c_double, ctypes.c_double, ctypes.c_double]
_lib.openstrut_set_restraint.restype = ctypes.c_int
_lib.openstrut_set_restraint.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int]
_lib.openstrut_add_material_steel.restype = ctypes.c_int
_lib.openstrut_add_material_steel.argtypes = [ctypes.c_void_p]
_lib.openstrut_add_material_concrete.restype = ctypes.c_int
_lib.openstrut_add_material_concrete.argtypes = [ctypes.c_void_p, ctypes.c_double]
_lib.openstrut_add_section_rectangular.restype = ctypes.c_int
_lib.openstrut_add_section_rectangular.argtypes = [ctypes.c_void_p, ctypes.c_double, ctypes.c_double]
_lib.openstrut_add_frame.restype = ctypes.c_int
_lib.openstrut_add_frame.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int]
_lib.openstrut_run_analysis.restype = ctypes.c_int
_lib.openstrut_run_analysis.argtypes = [ctypes.c_void_p]
_lib.openstrut_get_displacement.restype = ctypes.c_double
_lib.openstrut_get_displacement.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
_lib.openstrut_get_reaction.restype = ctypes.c_double
_lib.openstrut_get_reaction.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
_lib.openstrut_get_member_force.restype = ctypes.c_double
_lib.openstrut_get_member_force.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
_lib.openstrut_version.restype = ctypes.c_char_p


class Model:
    """OpenStrut structural model."""

    def __init__(self, name="Untitled"):
        self._handle = _lib.openstrut_model_new(name.encode())

    def __del__(self):
        if self._handle:
            _lib.openstrut_model_free(self._handle)

    def add_node(self, x, y, z):
        return _lib.openstrut_add_node(self._handle, x, y, z)

    def set_restraint(self, node_id, ux, uy, uz, rx, ry, rz):
        return _lib.openstrut_set_restraint(
            self._handle, node_id,
            int(ux), int(uy), int(uz), int(rx), int(ry), int(rz)
        )

    def add_material_steel(self):
        return _lib.openstrut_add_material_steel(self._handle)

    def add_material_concrete(self, fc=25.0):
        return _lib.openstrut_add_material_concrete(self._handle, fc)

    def add_section_rectangular(self, b, h):
        return _lib.openstrut_add_section_rectangular(self._handle, b, h)

    def add_frame(self, node_i, node_j, material_id, section_id):
        return _lib.openstrut_add_frame(
            self._handle, node_i, node_j, material_id, section_id
        )

    def run_analysis(self):
        return _lib.openstrut_run_analysis(self._handle) == 0

    def get_displacement(self, node_id, component=0):
        return _lib.openstrut_get_displacement(self._handle, node_id, component)

    def get_reaction(self, node_id, component=0):
        return _lib.openstrut_get_reaction(self._handle, node_id, component)

    def get_member_force(self, element_id, component=0):
        return _lib.openstrut_get_member_force(self._handle, element_id, component)

    @staticmethod
    def version():
        return _lib.openstrut_version().decode()


__all__ = ["Model"]