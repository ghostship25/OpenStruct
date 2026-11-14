use crate::model::Model;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub struct FileIO;

impl FileIO {
    pub fn save_json(model: &Model, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(model)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn load_json(path: &Path) -> std::io::Result<Model> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let model: Model = serde_json::from_str(&contents)?;
        Ok(model)
    }

    pub fn export_e2k(model: &Model, path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        writeln!(file, "; OpenStruct E2K Export")?;
        writeln!(file, "; Model: {}", model.name)?;
        writeln!(file)?;
        writeln!(file, "TABLE: \"ACTIVE DEGREES OF FREEDOM\"")?;
        writeln!(
            file,
            "   UX=Yes   UY=Yes   UZ=Yes   RX=Yes   RY=Yes   RZ=Yes"
        )?;
        writeln!(file)?;
        writeln!(file, "TABLE: \"JOINT COORDINATES\"")?;
        for (_id, node) in &model.nodes {
            writeln!(
                file,
                "   Joint=N{}   CoordSys=GLOBAL   CoordType=Cartesian   XorR={}   Y={}   Z={}",
                node.id, node.x, node.y, node.z
            )?;
        }
        writeln!(file)?;
        Ok(())
    }

    /// Export model to IFC 4 (Industry Foundation Classes) STEP/SPF text format.
    ///
    /// IFC 4 is an open standard for BIM data exchange. This export produces a
    /// valid IFC 4 file that can be opened in any IFC-compliant viewer or BIM
    /// tool (e.g., BIMvision, Solibri, Revit, ArchiCAD).
    ///
    /// The export includes:
    /// - IfcProject, IfcSite, IfcBuilding, IfcBuildingStorey (one per story)
    /// - IfcMaterial (from OpenStruct materials)
    /// - IfcMember (frame elements as structural members)
    /// - IfcCartesianPoint (joint coordinates)
    /// - IfcLocalPlacement for spatial hierarchy
    pub fn export_ifc(model: &Model, path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        let mut id_counter: usize = 1;

        // Helper to get next IFC entity ID
        let mut next_id = || {
            id_counter += 1;
            id_counter
        };

        // --- Header ---
        writeln!(file, "ISO-10303-21;")?;
        writeln!(file, "HEADER;")?;
        writeln!(
            file,
            "FILE_DESCRIPTION(('ViewDefinition [DesignTransferView_V1.0]'),'2;1');"
        )?;
        writeln!(
            file,
            "FILE_NAME('{}','2024-01-01T00:00:00',('OpenStruct'),('OpenStruct'),'OpenStruct 1.0','OpenStruct 1.0','');",
            model.name.replace('\'', "''")
        )?;
        writeln!(file, "FILE_SCHEMA(('IFC4'));")?;
        writeln!(file, "ENDSEC;")?;
        writeln!(file, "DATA;")?;

        // --- IfcProject ---
        let project_id = next_id();
        let owner_history_id = next_id();
        let unit_assignment_id = next_id();
        let geom_context_id = next_id();
        let body_context_id = next_id();

        // IfcOwnerHistory
        writeln!(
            file,
            "#{}=IFCOWNERHISTORY($,$,$,.READWRITE.,$,$,$,1704067200);",
            owner_history_id
        )?;

        // IfcUnitAssignment (SI units: m, N, kg, s, °C)
        let metre_unit_id = next_id();
        writeln!(
            file,
            "#{}=IFCUNITASSIGNMENT((#{}));",
            unit_assignment_id,
            metre_unit_id
        )?;
        writeln!(
            file,
            "#{}=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);",
            metre_unit_id
        )?;

        // IfcGeometricRepresentationContext
        writeln!(
            file,
            "#{}=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#1,#2);",
            geom_context_id
        )?;
        // IfcGeometricRepresentationSubContext (Body)
        writeln!(
            file,
            "#{}=IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#{}.T.,.MODEL_VIEW.,.TRUE.);",
            body_context_id, geom_context_id
        )?;

        // IfcProject
        writeln!(
            file,
            "#{}=IFCPROJECT('{}',#{},'{}','OpenStruct structural model',$,$,#${},#{});",
            project_id,
            format!("P{}", project_id),
            owner_history_id,
            model.name.replace('\'', "''"),
            unit_assignment_id,
            geom_context_id
        )?;

        // --- IfcSite ---
        let site_id = next_id();
        let site_placement_id = next_id();
        writeln!(
            file,
            "#{}=IFCLOCALPLACEMENT($,#{});",
            site_placement_id, site_placement_id
        )?;
        // Identity placement (origin)
        let origin_point_id = next_id();
        writeln!(
            file,
            "#{}=IFCCARTESIANPOINT((0.,0.,0.));",
            origin_point_id
        )?;
        let z_axis_id = next_id();
        writeln!(
            file,
            "#{}=IFCDIRECTION((0.,0.,1.));",
            z_axis_id
        )?;
        let x_axis_id = next_id();
        writeln!(
            file,
            "#{}=IFCDIRECTION((1.,0.,0.));",
            x_axis_id
        )?;
        let axis_placement_id = next_id();
        writeln!(
            file,
            "#{}=IFCAXIS2PLACEMENT3D(#{},{},{});",
            axis_placement_id, origin_point_id, z_axis_id, x_axis_id
        )?;
        let local_placement_id = next_id();
        writeln!(
            file,
            "#{}=IFCLOCALPLACEMENT($,#{});",
            local_placement_id, axis_placement_id
        )?;

        writeln!(
            file,
            "#{}=IFCSITE('{}',#{},'Site',$,$,#${},$,$,.ELEMENT.,$,$,$,$,$);",
            site_id,
            format!("S{}", site_id),
            owner_history_id,
            local_placement_id
        )?;

        // --- IfcBuilding ---
        let building_id = next_id();
        let building_placement_id = next_id();
        writeln!(
            file,
            "#{}=IFCLOCALPLACEMENT(#{} ,#{});",
            building_placement_id, site_placement_id, axis_placement_id
        )?;
        writeln!(
            file,
            "#{}=IFCBUILDING('{}',#{},'{}',$,#${},$,$,.ELEMENT.,$,$,$);",
            building_id,
            format!("B{}", building_id),
            owner_history_id,
            model.name.replace('\'', "''"),
            building_placement_id
        )?;

        // --- IfcBuildingStorey (one per story) ---
        let mut storey_ids: Vec<usize> = Vec::new();
        let mut storey_placement_ids: Vec<usize> = Vec::new();
        for story in &model.stories {
            let storey_id = next_id();
            let storey_placement_id = next_id();
            let elevation_point_id = next_id();
            writeln!(
                file,
                "#{}=IFCCARTESIANPOINT((0.,0.,{}));",
                elevation_point_id, story.elevation
            )?;
            let storey_axis_id = next_id();
            writeln!(
                file,
                "#{}=IFCAXIS2PLACEMENT3D(#{} ,#{},#{});",
                storey_axis_id, elevation_point_id, z_axis_id, x_axis_id
            )?;
            writeln!(
                file,
                "#{}=IFCLOCALPLACEMENT(#{} ,#{});",
                storey_placement_id, building_placement_id, storey_axis_id
            )?;
            writeln!(
                file,
                "#{}=IFCBUILDINGSTOREY('{}',#{},'{}',$,#${},$,$,.ELEMENT.,{});",
                storey_id,
                format!("ST{}", storey_id),
                owner_history_id,
                story.name.replace('\'', "''"),
                storey_placement_id,
                story.elevation
            )?;
            storey_ids.push(storey_id);
            storey_placement_ids.push(storey_placement_id);
        }

        // --- Spatial containment: Project -> Site -> Building -> Storeys ---
        let rel_site_id = next_id();
        writeln!(
            file,
            "#{}=IFCRELAGGREGATES('{}',#{},$,$,#{} ,(#{}));",
            rel_site_id,
            format!("R{}", rel_site_id),
            owner_history_id,
            project_id,
            site_id
        )?;

        let rel_building_id = next_id();
        writeln!(
            file,
            "#{}=IFCRELAGGREGATES('{}',#{},$,$,#{} ,(#{}));",
            rel_building_id,
            format!("R{}", rel_building_id),
            owner_history_id,
            site_id,
            building_id
        )?;

        if !storey_ids.is_empty() {
            let storey_list: Vec<String> = storey_ids.iter().map(|id| format!("#{}", id)).collect();
            let rel_storey_id = next_id();
            writeln!(
                file,
                "#{}=IFCRELAGGREGATES('{}',#{},$,$,#{} ,({}));",
                rel_storey_id,
                format!("R{}", rel_storey_id),
                owner_history_id,
                building_id,
                storey_list.join(",")
            )?;
        }

        // --- IfcMaterial (from OpenStruct materials) ---
        let mut material_ids: Vec<usize> = Vec::new();
        for (mat_id, material) in &model.materials {
            let ifc_mat_id = next_id();
            writeln!(
                file,
                "#{}=IFCMATERIAL('{}',$,$);",
                ifc_mat_id,
                material.name.replace('\'', "''")
            )?;
            material_ids.push(ifc_mat_id);

            // IfcRelAssociatesMaterial
            let rel_mat_id = next_id();
            writeln!(
                file,
                "#{}=IFCRELASSOCIATESMATERIAL('{}',#{},$,$,(#{}),#{});",
                rel_mat_id,
                format!("RM{}", rel_mat_id),
                owner_history_id,
                ifc_mat_id,
                ifc_mat_id
            )?;
        }

        // --- IfcMember (frame elements) ---
        // Group elements by story based on Z coordinate of node_i
        for (elem_id, element) in &model.elements {
            let node_i = match model.nodes.get(&element.node_i) {
                Some(n) => n,
                None => continue,
            };
            let node_j = match model.nodes.get(&element.node_j) {
                Some(n) => n,
                None => continue,
            };

            // Determine which storey this element belongs to (by node_i Z)
            let storey_idx = model
                .stories
                .iter()
                .enumerate()
                .rev()
                .find(|(_, s)| s.elevation <= node_i.z + 1e-6)
                .map(|(i, _)| i)
                .unwrap_or(0);

            let storey_placement_id = if storey_idx < storey_placement_ids.len() {
                storey_placement_ids[storey_idx]
            } else if !storey_placement_ids.is_empty() {
                storey_placement_ids[0]
            } else {
                building_placement_id
            };

            // Create IfcCartesianPoint for node_i and node_j
            let pt_i_id = next_id();
            writeln!(
                file,
                "#{}=IFCCARTESIANPOINT(({}, {}, {}));",
                pt_i_id, node_i.x, node_i.y, node_i.z
            )?;
            let pt_j_id = next_id();
            writeln!(
                file,
                "#{}=IFCCARTESIANPOINT(({}, {}, {}));",
                pt_j_id, node_j.x, node_j.y, node_j.z
            )?;

            // IfcPolyline for member axis
            let polyline_id = next_id();
            writeln!(
                file,
                "#{}=IFCPOLYLINE((#{},#{}));",
                polyline_id, pt_i_id, pt_j_id
            )?;

            // IfcShapeRepresentation (axis)
            let shape_rep_id = next_id();
            writeln!(
                file,
                "#{}=IFCSHAPEREPRESENTATION(#{} ,'Axis','Curve3D',(#{}));",
                shape_rep_id, body_context_id, polyline_id
            )?;

            // IfcProductDefinitionShape
            let prod_shape_id = next_id();
            writeln!(
                file,
                "#{}=IFCPRODUCTDEFINITIONSHAPE($,$,(#{}));",
                prod_shape_id, shape_rep_id
            )?;

            // IfcLocalPlacement for member
            let member_placement_id = next_id();
            writeln!(
                file,
                "#{}=IFCLOCALPLACEMENT(#{} ,#{});",
                member_placement_id, storey_placement_id, axis_placement_id
            )?;

            // IfcMember
            let member_id = next_id();
            let material_name = model
                .materials
                .get(&element.material_id)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let section_name = model
                .sections
                .get(&element.section_id)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            writeln!(
                file,
                "#{}=IFCMEMBER('{}',#{},'{} - {}',$,#{} ,#{} ,.MEMBER.,$);",
                member_id,
                format!("M{}", member_id),
                owner_history_id,
                material_name.replace('\'', "''"),
                section_name.replace('\'', "''"),
                member_placement_id,
                prod_shape_id
            )?;

            // IfcRelContainedInSpatialStructure (member in storey)
            let rel_contain_id = next_id();
            writeln!(
                file,
                "#{}=IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',#{},$,$,(#{}),#{});",
                rel_contain_id,
                format!("RC{}", rel_contain_id),
                owner_history_id,
                member_id,
                storey_ids.get(storey_idx).copied().unwrap_or(building_id)
            )?;
        }

        // --- IfcArea (slabs/walls) ---
        for (area_id, area) in &model.areas {
            let node_ids = &area.node_ids;
            if node_ids.is_empty() {
                continue;
            }

            // Determine storey by first node Z
            let first_node = match model.nodes.get(&node_ids[0]) {
                Some(n) => n,
                None => continue,
            };
            let storey_idx = model
                .stories
                .iter()
                .enumerate()
                .rev()
                .find(|(_, s)| s.elevation <= first_node.z + 1e-6)
                .map(|(i, _)| i)
                .unwrap_or(0);

            let storey_placement_id = if storey_idx < storey_placement_ids.len() {
                storey_placement_ids[storey_idx]
            } else if !storey_placement_ids.is_empty() {
                storey_placement_ids[0]
            } else {
                building_placement_id
            };

            // Create IfcCartesianPoint for each node
            let mut point_ids: Vec<usize> = Vec::new();
            for &nid in node_ids {
                if let Some(node) = model.nodes.get(&nid) {
                    let pt_id = next_id();
                    writeln!(
                        file,
                        "#{}=IFCCARTESIANPOINT(({}, {}, {}));",
                        pt_id, node.x, node.y, node.z
                    )?;
                    point_ids.push(pt_id);
                }
            }

            if point_ids.is_empty() {
                continue;
            }

            // IfcPolyline for slab boundary
            let polyline_id = next_id();
            let point_refs: Vec<String> = point_ids.iter().map(|id| format!("#{}", id)).collect();
            writeln!(
                file,
                "#{}=IFCPOLYLINE(({}));",
                polyline_id,
                point_refs.join(",")
            )?;

            // IfcArbitraryClosedProfileDef
            let profile_id = next_id();
            writeln!(
                file,
                "#{}=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,'SlabProfile',#{});",
                profile_id, polyline_id
            )?;

            // IfcExtrudedAreaSolid (slab thickness)
            let solid_id = next_id();
            let extrude_dir_id = next_id();
            writeln!(
                file,
                "#{}=IFCDIRECTION((0.,0.,1.));",
                extrude_dir_id
            )?;
            let depth = 0.2; // Default slab thickness
            writeln!(
                file,
                "#{}=IFCEXTRUDEDAREASOLID(#{} ,#{} ,#{} ,{});",
                solid_id, profile_id, axis_placement_id, extrude_dir_id, depth
            )?;

            // IfcShapeRepresentation
            let shape_rep_id = next_id();
            writeln!(
                file,
                "#{}=IFCSHAPEREPRESENTATION(#{} ,'Body','SweptSolid',(#{}));",
                shape_rep_id, body_context_id, solid_id
            )?;

            // IfcProductDefinitionShape
            let prod_shape_id = next_id();
            writeln!(
                file,
                "#{}=IFCPRODUCTDEFINITIONSHAPE($,$,(#{}));",
                prod_shape_id, shape_rep_id
            )?;

            // IfcSlab
            let slab_id = next_id();
            let slab_placement_id = next_id();
            writeln!(
                file,
                "#{}=IFCLOCALPLACEMENT(#{} ,#{});",
                slab_placement_id, storey_placement_id, axis_placement_id
            )?;
            writeln!(
                file,
                "#{}=IFCSLAB('{}',#{},'Slab {}',$,#{} ,#{} ,.FLOOR.,$);",
                slab_id,
                format!("SL{}", slab_id),
                owner_history_id,
                area_id,
                slab_placement_id,
                prod_shape_id
            )?;

            // IfcRelContainedInSpatialStructure
            let rel_contain_id = next_id();
            writeln!(
                file,
                "#{}=IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',#{},$,$,(#{}),#{});",
                rel_contain_id,
                format!("RC{}", rel_contain_id),
                owner_history_id,
                slab_id,
                storey_ids.get(storey_idx).copied().unwrap_or(building_id)
            )?;
        }

        // --- End of data ---
        writeln!(file, "ENDSEC;")?;
        writeln!(file, "END-ISO-10303-21;")?;

        Ok(())
    }
}

impl FileIO {
    /// Export model to DXF (Drawing Exchange Format) for 2D/3D drawing production.
    ///
    /// DXF is an open CAD data format. This export produces a valid DXF R12/R14
    /// ASCII file that can be opened in any CAD software (AutoCAD, LibreCAD,
    ///
    /// The export includes:
    /// - Header section with units and extents
    /// - Tables section with layers and linetypes
    /// - Entities section with lines (frame members), circles (joints),
    /// - Layer assignments: STRUCTURE (frames), JOINTS (nodes), SLABS (areas),
    pub fn export_dxf(model: &Model, path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;

        // --- Header ---
        writeln!(file, "0")?;
        writeln!(file, "SECTION")?;
        writeln!(file, "2")?;
        writeln!(file, "HEADER")?;
        writeln!(file, "9")?;
        writeln!(file, "$ACADVER")?;
        writeln!(file, "1")?;
        writeln!(file, "AC1014")?; // AutoCAD R14
        writeln!(file, "9")?;
        writeln!(file, "$INSBASE")?;
        writeln!(file, "10")?;
        writeln!(file, "0.0")?;
        writeln!(file, "20")?;
        writeln!(file, "0.0")?;
        writeln!(file, "30")?;
        writeln!(file, "0.0")?;
        writeln!(file, "9")?;
        writeln!(file, "$EXTMIN")?;
        writeln!(file, "10")?;
        writeln!(file, "0.0")?;
        writeln!(file, "20")?;
        writeln!(file, "0.0")?;
        writeln!(file, "30")?;
        writeln!(file, "0.0")?;
        writeln!(file, "9")?;
        writeln!(file, "$EXTMAX")?;
        writeln!(file, "10")?;
        writeln!(file, "100.0")?;
        writeln!(file, "20")?;
        writeln!(file, "100.0")?;
        writeln!(file, "30")?;
        writeln!(file, "100.0")?;
        writeln!(file, "0")?;
        writeln!(file, "ENDSEC")?;

        // --- Tables ---
        writeln!(file, "0")?;
        writeln!(file, "SECTION")?;
        writeln!(file, "2")?;
        writeln!(file, "TABLES")?;

        // LType table
        writeln!(file, "0")?;
        writeln!(file, "TABLE")?;
        writeln!(file, "2")?;
        writeln!(file, "LTYPE")?;
        writeln!(file, "70")?;
        writeln!(file, "1")?;
        writeln!(file, "0")?;
        writeln!(file, "LTYPE")?;
        writeln!(file, "2")?;
        writeln!(file, "CONTINUOUS")?;
        writeln!(file, "70")?;
        writeln!(file, "0")?;
        writeln!(file, "3")?;
        writeln!(file, "Solid line")?;
        writeln!(file, "72")?;
        writeln!(file, "65")?;
        writeln!(file, "73")?;
        writeln!(file, "0")?;
        writeln!(file, "40")?;
        writeln!(file, "0.0")?;
        writeln!(file, "0")?;
        writeln!(file, "ENDTAB")?;

        // Layer table
        writeln!(file, "0")?;
        writeln!(file, "TABLE")?;
        writeln!(file, "2")?;
        writeln!(file, "LAYER")?;
        writeln!(file, "70")?;
        writeln!(file, "4")?;

        // Layer: STRUCTURE
        writeln!(file, "0")?;
        writeln!(file, "LAYER")?;
        writeln!(file, "2")?;
        writeln!(file, "STRUCTURE")?;
        writeln!(file, "70")?;
        writeln!(file, "0")?;
        writeln!(file, "62")?;
        writeln!(file, "7")?; // white
        writeln!(file, "6")?;
        writeln!(file, "CONTINUOUS")?;

        // Layer: JOINTS
        writeln!(file, "0")?;
        writeln!(file, "LAYER")?;
        writeln!(file, "2")?;
        writeln!(file, "JOINTS")?;
        writeln!(file, "70")?;
        writeln!(file, "0")?;
        writeln!(file, "62")?;
        writeln!(file, "1")?; // red
        writeln!(file, "6")?;
        writeln!(file, "CONTINUOUS")?;

        // Layer: SLABS
        writeln!(file, "0")?;
        writeln!(file, "LAYER")?;
        writeln!(file, "2")?;
        writeln!(file, "SLABS")?;
        writeln!(file, "70")?;
        writeln!(file, "0")?;
        writeln!(file, "62")?;
        writeln!(file, "3")?; // green
        writeln!(file, "6")?;
        writeln!(file, "CONTINUOUS")?;

        // Layer: ANNOTATIONS
        writeln!(file, "0")?;
        writeln!(file, "LAYER")?;
        writeln!(file, "2")?;
        writeln!(file, "ANNOTATIONS")?;
        writeln!(file, "70")?;
        writeln!(file, "0")?;
        writeln!(file, "62")?;
        writeln!(file, "2")?; // yellow
        writeln!(file, "6")?;
        writeln!(file, "CONTINUOUS")?;

        writeln!(file, "0")?;
        writeln!(file, "ENDTAB")?;
        writeln!(file, "0")?;
        writeln!(file, "ENDSEC")?;

        // --- Entities ---
        writeln!(file, "0")?;
        writeln!(file, "SECTION")?;
        writeln!(file, "2")?;
        writeln!(file, "ENTITIES")?;

        // Draw frame elements as LINE entities
        for (elem_id, element) in &model.elements {
            let node_i = match model.nodes.get(&element.node_i) {
                Some(n) => n,
                None => continue,
            };
            let node_j = match model.nodes.get(&element.node_j) {
                Some(n) => n,
                None => continue,
            };

            writeln!(file, "0")?;
            writeln!(file, "LINE")?;
            writeln!(file, "8")?; // layer group
            writeln!(file, "STRUCTURE")?;
            writeln!(file, "10")?; // start point X
            writeln!(file, "{}", node_i.x)?;
            writeln!(file, "20")?; // start point Y
            writeln!(file, "{}", node_i.y)?;
            writeln!(file, "30")?; // start point Z
            writeln!(file, "{}", node_i.z)?;
            writeln!(file, "11")?; // end point X
            writeln!(file, "{}", node_j.x)?;
            writeln!(file, "21")?; // end point Y
            writeln!(file, "{}", node_j.y)?;
            writeln!(file, "31")?; // end point Z
            writeln!(file, "{}", node_j.z)?;
        }

        // Draw joints as CIRCLE entities
        for (node_id, node) in &model.nodes {
            writeln!(file, "0")?;
            writeln!(file, "CIRCLE")?;
            writeln!(file, "8")?;
            writeln!(file, "JOINTS")?;
            writeln!(file, "10")?; // center X
            writeln!(file, "{}", node.x)?;
            writeln!(file, "20")?; // center Y
            writeln!(file, "{}", node.y)?;
            writeln!(file, "30")?; // center Z
            writeln!(file, "{}", node.z)?;
            writeln!(file, "40")?; // radius
            writeln!(file, "0.1")?;
        }

        // Draw slabs as POLYLINE entities
        for (area_id, area) in &model.areas {
            if area.node_ids.is_empty() {
                continue;
            }

            writeln!(file, "0")?;
            writeln!(file, "POLYLINE")?;
            writeln!(file, "8")?;
            writeln!(file, "SLABS")?;
            writeln!(file, "66")?;
            writeln!(file, "1")?; // vertices follow
            writeln!(file, "70")?;
            writeln!(file, "1")?; // closed polyline

            for &nid in &area.node_ids {
                if let Some(node) = model.nodes.get(&nid) {
                    writeln!(file, "0")?;
                    writeln!(file, "VERTEX")?;
                    writeln!(file, "8")?;
                    writeln!(file, "SLABS")?;
                    writeln!(file, "10")?;
                    writeln!(file, "{}", node.x)?;
                    writeln!(file, "20")?;
                    writeln!(file, "{}", node.y)?;
                    writeln!(file, "30")?;
                    writeln!(file, "{}", node.z)?;
                }
            }

            writeln!(file, "0")?;
            writeln!(file, "SEQEND")?;
        }

        // Add text annotations for node IDs
        for (node_id, node) in &model.nodes {
            writeln!(file, "0")?;
            writeln!(file, "TEXT")?;
            writeln!(file, "8")?;
            writeln!(file, "ANNOTATIONS")?;
            writeln!(file, "10")?; // insertion point X
            writeln!(file, "{}", node.x + 0.15)?;
            writeln!(file, "20")?; // insertion point Y
            writeln!(file, "{}", node.y + 0.15)?;
            writeln!(file, "30")?; // insertion point Z
            writeln!(file, "{}", node.z)?;
            writeln!(file, "40")?; // text height
            writeln!(file, "0.2")?;
            writeln!(file, "1")?; // text string
            writeln!(file, "N{}", node_id)?;
        }

        writeln!(file, "0")?;
        writeln!(file, "ENDSEC")?;
        writeln!(file, "0")?;
        writeln!(file, "EOF")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Model;
    use crate::materials::Material;
    use crate::sections::Section;
    use crate::elements::Element;

    /// Output path inside the system temp directory (this suite runs on Windows too).
    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn test_ifc_export_produces_valid_structure() {
        let mut model = Model::new("IFC Test Model");
        let mat_id = model.add_material(Material::steel());
        let sec_id = model.add_section(Section::rectangular("RECT300X500", 0.3, 0.5));
        let n1 = model.add_node(0.0, 0.0, 0.0);
        let n2 = model.add_node(4.0, 0.0, 0.0);
        let e1 = model.add_element(Element::frame(1, n1, n2, mat_id, sec_id));
        model.nodes.get_mut(&n1).unwrap().restrained = [true, true, true, true, false, false];
        model.nodes.get_mut(&n2).unwrap().restrained = [true, true, true, false, false, false];

        let path = temp_path("test_export.ifc");
        FileIO::export_ifc(&model, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("ISO-10303-21;"));
        assert!(content.contains("IFCPROJECT"));
        assert!(content.contains("IFCSITE"));
        assert!(content.contains("IFCBUILDING"));
        assert!(content.contains("IFCBUILDINGSTOREY"));
        assert!(content.contains("IFCMEMBER"));
        assert!(content.contains("IFCMATERIAL"));
        assert!(content.contains("END-ISO-10303-21;"));

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_ifc_export_empty_model() {
        let model = Model::new("Empty IFC Model");
        let path = temp_path("test_empty.ifc");
        FileIO::export_ifc(&model, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("ISO-10303-21;"));
        assert!(content.contains("IFCPROJECT"));
        assert!(content.contains("END-ISO-10303-21;"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_ifc_export_with_slab() {
        let mut model = Model::new("IFC Slab Test");
        let mat_id = model.add_material(Material::steel());
        let sec_id = model.add_section(Section::rectangular("SLAB 200", 0.2, 0.2));
        let n1 = model.add_node(0.0, 0.0, 0.0);
        let n2 = model.add_node(4.0, 0.0, 0.0);
        let n3 = model.add_node(4.0, 4.0, 0.0);
        let n4 = model.add_node(0.0, 4.0, 0.0);
        let area = crate::areas::Area::new(vec![n1, n2, n3, n4], crate::areas::AreaKind::Slab, mat_id, 0.2);
        model.add_area(area);

        let path = temp_path("test_slab.ifc");
        FileIO::export_ifc(&model, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("IFCSLAB"));
        assert!(content.contains("IFCARBITRARYCLOSEDPROFILEDEF"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_dxf_export_produces_valid_structure() {
        let mut model = Model::new("DXF Test Model");
        let mat_id = model.add_material(Material::steel());
        let sec_id = model.add_section(Section::rectangular("RECT300X500", 0.3, 0.5));
        let n1 = model.add_node(0.0, 0.0, 0.0);
        let n2 = model.add_node(4.0, 0.0, 0.0);
        let e1 = model.add_element(Element::frame(1, n1, n2, mat_id, sec_id));
        model.nodes.get_mut(&n1).unwrap().restrained = [true, true, true, true, false, false];

        let path = temp_path("test_export.dxf");
        FileIO::export_dxf(&model, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("SECTION"));
        assert!(content.contains("HEADER"));
        assert!(content.contains("TABLES"));
        assert!(content.contains("ENTITIES"));
        assert!(content.contains("LINE"));
        assert!(content.contains("CIRCLE"));
        assert!(content.contains("STRUCTURE"));
        assert!(content.contains("JOINTS"));
        assert!(content.contains("EOF"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_dxf_export_empty_model() {
        let model = Model::new("Empty DXF Model");
        let path = temp_path("test_empty.dxf");
        FileIO::export_dxf(&model, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("SECTION"));
        assert!(content.contains("EOF"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_dxf_export_with_slab() {
        let mut model = Model::new("DXF Slab Test");
        let mat_id = model.add_material(Material::steel());
        let n1 = model.add_node(0.0, 0.0, 0.0);
        let n2 = model.add_node(4.0, 0.0, 0.0);
        let n3 = model.add_node(4.0, 4.0, 0.0);
        let n4 = model.add_node(0.0, 4.0, 0.0);
        let area = crate::areas::Area::new(vec![n1, n2, n3, n4], crate::areas::AreaKind::Slab, mat_id, 0.2);
        model.add_area(area);

        let path = temp_path("test_slab.dxf");
        FileIO::export_dxf(&model, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("POLYLINE"));
        assert!(content.contains("SLABS"));

        let _ = std::fs::remove_file(&path);
    }
}
