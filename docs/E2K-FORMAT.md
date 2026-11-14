# E2K text format support

OpenStruct reads and writes a line-based `.e2k` text model. The reader is
independent of any particular desktop package: it consumes documented text
records only and never reads a binary model database.

## Records read

| Area | Records |
| --- | --- |
| Units | `UNITS` |
| Geometry | `STORY`, `POINT`, `LINE`, `AREA`, `POINTASSIGN`, `LINEASSIGN`, `AREAASSIGN` |
| Properties | `MATERIAL`, `FRAMESECTION`, `SHELLPROP` |
| Supports | `POINTASSIGN` restraint flags |
| Diaphragms | `DIAPHRAGM` assignments |
| Loads | `LOADPATTERN`, `LINELOAD`, `AREALOAD` |
| Cases | `LOADCASE` (linear static only) |
| Combinations | `COMBO` (linear add) |

Units are read from the file and converted to SI. Storey heights, master
stories and similar-to stories are preserved. Frame sections retain their first
auto-select shape; section-designer shapes are not reconstructed.

## Records written

`export_e2k` writes a readable subset: units, stories, materials, frame
sections, points, lines, areas, supports, diaphragm assignments, load patterns,
linear static cases with joint/frame/area loads, and linear-add combinations.

## Gaps reported at import

Every import returns a report with record counts and warnings. Current gaps:

- Modal, response-spectrum and other non-static cases are skipped.
- Auto wind and seismic loads are not present in exported text and are not
  generated, so such cases solve to zero and are reported.
- Auto-select section lists keep only their first shape.
- Composite deck, staged construction, links and tendons are not imported.
- Shell pressures are approximated as perimeter frame line loads because the
  solver has no shell element.
- A small number of preference records are not used by the model.

The report is shown in the desktop application before analysis and is also
available from the headless CLI.
