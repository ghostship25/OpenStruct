//! Linear static analysis: stiffness assembly, load assembly, solve, member end
//! forces, load-combination evaluation and design envelopes.

use crate::elements::Element;
use crate::fem;
use crate::loads::{ComboType, LoadCase, LoadDirection, LoadType};
use crate::model::Model;
use nalgebra::{DMatrix, DVector};
use std::collections::{HashMap, HashSet};

/// Degrees of freedom per joint.
pub const DOFS_PER_JOINT: usize = 6;
/// Local end forces per member.
pub const FORCES_PER_MEMBER: usize = 12;

/// Solved displacements, reactions and member end forces for one load case or combination.
#[derive(Debug, Clone, Default)]
pub struct AnalysisResults {
    pub status: String,
    pub node_ids: Vec<usize>,
    /// 6 values per joint in `node_ids` order: Ux, Uy, Uz, Rx, Ry, Rz.
    pub displacements: Vec<f64>,
    /// 6 values per joint in `node_ids` order.
    pub reactions: Vec<f64>,
    pub element_ids: Vec<usize>,
    /// 12 values per member in `element_ids` order (local end forces i then j).
    pub member_forces: Vec<f64>,
}

impl AnalysisResults {
    fn joint_index(&self, node_id: usize) -> Option<usize> {
        self.node_ids.iter().position(|&id| id == node_id)
    }

    pub fn displacement_of(&self, node_id: usize) -> Option<[f64; 6]> {
        let index = self.joint_index(node_id)? * DOFS_PER_JOINT;
        let mut out = [0.0; 6];
        for (dof, slot) in out.iter_mut().enumerate() {
            *slot = *self.displacements.get(index + dof)?;
        }
        Some(out)
    }

    pub fn reactions_of(&self, node_id: usize) -> Option<[f64; 6]> {
        let index = self.joint_index(node_id)? * DOFS_PER_JOINT;
        let mut out = [0.0; 6];
        for (dof, slot) in out.iter_mut().enumerate() {
            *slot = *self.reactions.get(index + dof)?;
        }
        Some(out)
    }

    pub fn member_forces_of(&self, element_id: usize) -> Option<[f64; 12]> {
        let index = self.element_ids.iter().position(|&id| id == element_id)? * FORCES_PER_MEMBER;
        let mut out = [0.0; 12];
        for (slot_index, slot) in out.iter_mut().enumerate() {
            *slot = *self.member_forces.get(index + slot_index)?;
        }
        Some(out)
    }
}

/// Running maximum and minimum of one response component.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extremes {
    pub max: f64,
    pub min: f64,
}

impl Default for Extremes {
    fn default() -> Self {
        Self { max: f64::NEG_INFINITY, min: f64::INFINITY }
    }
}

impl Extremes {
    pub fn add(&mut self, value: f64) {
        if value > self.max {
            self.max = value;
        }
        if value < self.min {
            self.min = value;
        }
    }

    /// Peak absolute value, with the sign of the extreme that produced it.
    pub fn peak(&self) -> f64 {
        if self.max == f64::NEG_INFINITY {
            0.0
        } else if self.max.abs() >= self.min.abs() {
            self.max
        } else {
            self.min
        }
    }
}

/// Per-member response envelope across every evaluated combination.
#[derive(Debug, Clone, Default)]
pub struct MemberEnvelope {
    pub element_id: usize,
    pub axial: Extremes,
    pub moment_y_i: Extremes,
    pub moment_y_j: Extremes,
    pub moment_z_i: Extremes,
    pub moment_z_j: Extremes,
    pub shear_y: Extremes,
    pub shear_z: Extremes,
    pub twist_i: Extremes,
    pub twist_j: Extremes,
}

/// One evaluated load combination.
#[derive(Debug, Clone)]
pub struct CombinationResult {
    pub name: String,
    pub combo_type: ComboType,
    pub equation: String,
    pub status: String,
    pub max_displacement: f64,
    pub max_reaction: f64,
    pub results: AnalysisResults,
}

/// Collect design envelopes for every member that appears in any result set.
pub fn member_envelopes(sets: &[&AnalysisResults]) -> Vec<MemberEnvelope> {
    let mut envelopes: Vec<MemberEnvelope> = Vec::new();
    for set in sets {
        for (position, &element_id) in set.element_ids.iter().enumerate() {
            let base = position * FORCES_PER_MEMBER;
            let force = |offset: usize| set.member_forces.get(base + offset).copied().unwrap_or(0.0);
            let entry = match envelopes.iter_mut().find(|entry| entry.element_id == element_id) {
                Some(entry) => entry,
                None => {
                    envelopes.push(MemberEnvelope { element_id, ..Default::default() });
                    envelopes.last_mut().expect("just pushed")
                }
            };
            entry.axial.add(force(0));
            entry.axial.add(force(6));
            entry.shear_y.add(force(1));
            entry.shear_y.add(force(7));
            entry.shear_z.add(force(2));
            entry.shear_z.add(force(8));
            entry.twist_i.add(force(3));
            entry.twist_j.add(force(9));
            entry.moment_y_i.add(force(4));
            entry.moment_y_j.add(force(10));
            entry.moment_z_i.add(force(5));
            entry.moment_z_j.add(force(11));
        }
    }
    envelopes.sort_by_key(|entry| entry.element_id);
    envelopes
}

/// A model plus the load cases to solve.
pub struct Analysis {
    pub model: Model,
    pub load_cases: Vec<LoadCase>,
}

impl Analysis {
    pub fn new(model: Model) -> Self {
        let load_cases = if model.load_cases.is_empty() {
            vec![default_case()]
        } else {
            model.load_cases.clone()
        };
        Self { model, load_cases }
    }

    /// The case solved by `run_static`: the first case, which is what the
    /// Displacements/Reactions/Forces tables show before a combination is picked.
    pub fn first_case(&self) -> Option<&LoadCase> {
        self.load_cases.first()
    }
}

/// Fallback case used when a model carries no load cases yet.
pub fn default_case() -> LoadCase {
    LoadCase {
        id: 1,
        name: "CASE 1".into(),
        pattern: LoadType::Dead,
        self_weight_multiplier: 1.0,
        nodal_loads: Vec::new(),
        distributed_loads: Vec::new(),
        p_delta: false,
    }
}

/// One member prepared for assembly.
struct Member {
    element_id: usize,
    dofs: [usize; 12],
    k_local: DMatrix<f64>,
    t: DMatrix<f64>,
    r: DMatrix<f64>,
    length: f64,
    area: f64,
    modulus: f64,
    alpha: f64,
}

impl Analysis {
    fn node_index(&self) -> (Vec<usize>, HashMap<usize, usize>) {
        let mut ids: Vec<usize> = self.model.nodes.keys().copied().collect();
        ids.sort_unstable();
        let map: HashMap<usize, usize> = ids.iter().enumerate().map(|(index, &id)| (id, index)).collect();
        (ids, map)
    }

    fn prepare_member(&self, element: &Element, index: &HashMap<usize, usize>) -> Option<Member> {
        let node_i = self.model.nodes.get(&element.node_i)?;
        let node_j = self.model.nodes.get(&element.node_j)?;
        let material = self.model.materials.get(&element.material_id)?;
        let section = self.model.sections.get(&element.section_id)?;
        let (i_index, j_index) = (index.get(&element.node_i)?, index.get(&element.node_j)?);
        let length = fem::element_length(node_i.x, node_i.y, node_i.z, node_j.x, node_j.y, node_j.z);
        if !(length.is_finite() && length > 1e-9) {
            return None;
        }
        let area = section.area.max(1e-12);
        let modulus = if material.e > 0.0 { material.e } else { 200.0e9 };
        let shear = if material.g > 0.0 { material.g } else { modulus / (2.0 * (1.0 + material.nu.max(0.0))) };
        let k_local = fem::local_stiffness_frame(
            modulus,
            area,
            section.iz.max(1e-12),
            section.iy.max(1e-12),
            shear,
            section.j.max(1e-12),
            length,
        );
        let condensed = fem::ReleasedMember::condense(&k_local, &element.releases, &element.partial_fixity);
        let r = fem::frame_rotation(
            (node_j.x - node_i.x) / length,
            (node_j.y - node_i.y) / length,
            (node_j.z - node_i.z) / length,
        );
        let t = fem::transformation(&r);
        let base_i = *i_index * DOFS_PER_JOINT;
        let base_j = *j_index * DOFS_PER_JOINT;
        let mut dofs = [0usize; 12];
        for (offset, slot) in dofs.iter_mut().enumerate() {
            *slot = if offset < DOFS_PER_JOINT { base_i + offset } else { base_j + offset - DOFS_PER_JOINT };
        }
        Some(Member {
            element_id: element.id,
            dofs,
            k_local: condensed.stiffness,
            t,
            r,
            length,
            area,
            modulus,
            alpha: material.alpha,
        })
    }

    /// Assemble the global stiffness matrix and load vector for one case.
    fn assemble(&self, case: &LoadCase) -> (Vec<usize>, HashMap<usize, usize>, DMatrix<f64>, DVector<f64>, Vec<Member>, HashMap<usize, DVector<f64>>) {
        let (ids, index) = self.node_index();
        let ndof = ids.len() * DOFS_PER_JOINT;
        let mut stiffness = DMatrix::<f64>::zeros(ndof, ndof);
        let mut force = DVector::<f64>::zeros(ndof);

        let mut element_ids: Vec<usize> = self.model.elements.keys().copied().collect();
        element_ids.sort_unstable();
        let mut members: Vec<Member> = Vec::new();
        for element_id in element_ids {
            let element = match self.model.elements.get(&element_id) {
                Some(element) => element,
                None => continue,
            };
            let member = match self.prepare_member(element, &index) {
                Some(member) => member,
                None => continue,
            };
            let k_global = member.t.transpose() * &member.k_local * &member.t;
            for row in 0..12 {
                let dof_row = member.dofs[row];
                for column in 0..12 {
                    stiffness[(dof_row, member.dofs[column])] += k_global[(row, column)];
                }
            }
            members.push(member);
        }

        // Support springs act on the global diagonal.
        for (&node_id, node) in &self.model.nodes {
            if let Some(&joint) = index.get(&node_id) {
                for (dof, &spring) in node.springs.iter().enumerate() {
                    if spring > 0.0 && !node.restrained[dof] {
                        let slot = joint * DOFS_PER_JOINT + dof;
                        stiffness[(slot, slot)] += spring;
                    }
                }
            }
        }

        let first_case = self.load_cases.first().map(|c| c.id) == Some(case.id) || self.load_cases.len() <= 1;
        let mut fixed_end: HashMap<usize, DVector<f64>> = HashMap::new();

        // Self weight: rho * A * g acting along global -Z, half to each joint.
        if case.self_weight_multiplier != 0.0 {
            for member in &members {
                let density = self
                    .model
                    .elements
                    .get(&member.element_id)
                    .and_then(|element| self.model.materials.get(&element.material_id))
                    .map(|material| material.rho)
                    .unwrap_or(0.0);
                let weight = density * member.area * 9.80665 * member.length * case.self_weight_multiplier;
                if weight == 0.0 {
                    continue;
                }
                force[member.dofs[2]] -= weight / 2.0;
                force[member.dofs[8]] -= weight / 2.0;
            }
        }

        // Assigned point loads.
        for load in &case.nodal_loads {
            if let Some(&joint) = index.get(&load.node_id) {
                let base = joint * DOFS_PER_JOINT;
                force[base] += load.fx;
                force[base + 1] += load.fy;
                force[base + 2] += load.fz;
                force[base + 3] += load.mx;
                force[base + 4] += load.my;
                force[base + 5] += load.mz;
            }
        }

        // Distributed member loads: equivalent (consistent) nodal loads.
        for load in self
            .model
            .frame_loads
            .iter()
            .filter(|load| load.case_id == Some(case.id) || (load.case_id.is_none() && first_case))
        {
            let member = match members.iter().find(|member| member.element_id == load.element_id) {
                Some(member) => member,
                None => continue,
            };
            let (start, end) = match load.direction {
                LoadDirection::Global => {
                    let q_start = member.r.clone() * DVector::from_row_slice(&load.start);
                    let q_end = member.r.clone() * DVector::from_row_slice(&load.end);
                    ([q_start[0], q_start[1], q_start[2]], [q_end[0], q_end[1], q_end[2]])
                }
                LoadDirection::Local => (load.start, load.end),
            };
            let local = fem::local_distributed_load(start, end, member.length);
            let global = member.t.transpose() * &local;
            for row in 0..12 {
                force[member.dofs[row]] += global[row];
            }
            fixed_end
                .entry(member.element_id)
                .and_modify(|existing| *existing += &local)
                .or_insert(local);
        }

        // Area pressures: the solver has no shell elements, so an area's total
        // pressure is transferred to its perimeter frame members as a uniform
        // global -Z line load, shared equally along the supported edge length.
        let mut edge_member: HashMap<(usize, usize), (usize, f64)> = HashMap::new();
        for element in self.model.elements.values() {
            let (Some(a), Some(b)) = (self.model.nodes.get(&element.node_i), self.model.nodes.get(&element.node_j)) else {
                continue;
            };
            if (a.z - b.z).abs() > 1.0e-6 {
                continue;
            }
            let length = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            if length <= 1.0e-9 || element.id == 0 {
                continue;
            }
            let key = (element.node_i.min(element.node_j), element.node_i.max(element.node_j));
            edge_member.entry(key).or_insert((element.id, length));
        }
        let mut area_line: HashMap<usize, [f64; 3]> = HashMap::new();
        for load in self
            .model
            .area_loads
            .iter()
            .filter(|load| load.case_id == Some(case.id) || (load.case_id.is_none() && first_case))
        {
            let Some(area) = self.model.areas.get(&load.area_id) else { continue };
            let loaded_area = area.area(&self.model);
            if !(loaded_area.is_finite() && loaded_area > 0.0) {
                continue;
            }
            let nodes = &area.node_ids;
            if nodes.len() < 3 {
                continue;
            }
            let mut supported = Vec::new();
            let mut supported_length = 0.0;
            for index in 0..nodes.len() {
                let a = nodes[index];
                let b = nodes[(index + 1) % nodes.len()];
                let key = (a.min(b), a.max(b));
                if let Some(&(element_id, length)) = edge_member.get(&key) {
                    supported.push(element_id);
                    supported_length += length;
                }
            }
            if supported_length <= 1.0e-9 {
                continue;
            }
            let intensity = load.pressure.abs() * loaded_area / supported_length;
            for element_id in supported {
                area_line.entry(element_id).or_insert([0.0; 3])[2] -= intensity;
            }
        }
        for (element_id, load) in area_line {
            let Some(member) = members.iter().find(|member| member.element_id == element_id) else {
                continue;
            };
            let q_start = member.r.clone() * DVector::from_row_slice(&load);
            let q_end = member.r.clone() * DVector::from_row_slice(&load);
            let local = fem::local_distributed_load(
                [q_start[0], q_start[1], q_start[2]],
                [q_end[0], q_end[1], q_end[2]],
                member.length,
            );
            let global = member.t.transpose() * &local;
            for row in 0..12 {
                force[member.dofs[row]] += global[row];
            }
            fixed_end
                .entry(member.element_id)
                .and_modify(|existing| *existing += &local)
                .or_insert(local);
        }

        // Temperature / initial strain: equivalent axial end forces.
        for load in self
            .model
            .temperature_loads
            .iter()
            .filter(|load| load.case_id == Some(case.id) || (load.case_id.is_none() && first_case))
        {
            let member = match members.iter().find(|member| member.element_id == load.element_id) {
                Some(member) => member,
                None => continue,
            };
            let axial = member.modulus * member.area * load.axial_strain(member.alpha);
            let mut local = DVector::<f64>::zeros(12);
            local[0] = -axial;
            local[6] = axial;
            let global = member.t.transpose() * &local;
            for row in 0..12 {
                force[member.dofs[row]] += global[row];
            }
            fixed_end
                .entry(member.element_id)
                .and_modify(|existing| *existing += &local)
                .or_insert(local);
        }

        (ids, index, stiffness, force, members, fixed_end)
    }

    /// Solve one load case and return displacements, reactions and member forces.
    pub fn run_case(&self, case: &LoadCase) -> AnalysisResults {
        let (ids, index, stiffness, force, members, fixed_end) = self.assemble(case);
        let ndof = stiffness.nrows();
        let mut fixed: Vec<usize> = Vec::new();
        for (node_id, node) in &self.model.nodes {
            if let Some(&joint) = index.get(node_id) {
                for (dof, &restrained) in node.restrained.iter().enumerate() {
                    if restrained {
                        fixed.push(joint * DOFS_PER_JOINT + dof);
                    }
                }
            }
        }
        fixed.sort_unstable();

        let free: Vec<usize> = (0..ndof).filter(|dof| fixed.binary_search(dof).is_err()).collect();
        // Joints that belong only to areas (or to nothing) carry no frame
        // stiffness, and a frame island with no restraint is a free body. With
        // no shell element in the solver both would make the whole model
        // singular, so their DOFs are left at zero displacement and reported in
        // the status instead of failing the supported frame.
        fn root(parent: &mut [usize], mut joint: usize) -> usize {
            while parent[joint] != joint {
                parent[joint] = parent[parent[joint]];
                joint = parent[joint];
            }
            joint
        }
        let joint_count = ids.len();
        let mut parent: Vec<usize> = (0..joint_count).collect();
        for member in &members {
            let (a, b) = (member.dofs[0] / DOFS_PER_JOINT, member.dofs[6] / DOFS_PER_JOINT);
            let (root_a, root_b) = (root(&mut parent, a), root(&mut parent, b));
            if root_a != root_b {
                parent[root_a] = root_b;
            }
        }
        let mut supported_roots: HashSet<usize> = HashSet::new();
        for (node_id, node) in &self.model.nodes {
            if let Some(&joint) = index.get(node_id) {
                if node.restrained.iter().any(|restrained| *restrained) || node.springs.iter().any(|spring| *spring > 0.0) {
                    supported_roots.insert(root(&mut parent, joint));
                }
            }
        }
        let connected: Vec<bool> = (0..joint_count)
            .map(|joint| supported_roots.contains(&root(&mut parent, joint)))
            .collect();
        let active: Vec<usize> = free
            .iter()
            .copied()
            .filter(|&dof| connected[dof / DOFS_PER_JOINT] && (0..ndof).any(|column| stiffness[(dof, column)] != 0.0))
            .collect();
        let skipped = free.len() - active.len();
        let reduced = DMatrix::from_fn(active.len(), active.len(), |i, j| stiffness[(active[i], active[j])]);
        let reduced_force = DVector::from_vec(active.iter().map(|&dof| force[dof]).collect::<Vec<f64>>());
        // An empty reduced system (every joint restrained) is already solved;
        // nalgebra's LU panics on a 0x0 matrix.
        let solution = if active.is_empty() {
            Some(DVector::<f64>::zeros(0))
        } else {
            reduced.lu().solve(&reduced_force)
        };

        let mut displacements = DVector::<f64>::zeros(ndof);
        let mut status = String::from("Solved");
        match solution {
            Some(values) => {
                for (row, &dof) in active.iter().enumerate() {
                    displacements[dof] = values[row];
                }
                if skipped > 0 {
                    status = format!(
                        "Solved; {skipped} dof were left at zero (joints with no supported frame: shell-only joints or unsupported islands; no shell element in the solver)"
                    );
                }
            }
            None => {
                status = "Singular stiffness matrix - check supports and releases".to_string();
            }
        }

        let residual = &stiffness * &displacements - &force;
        let mut reactions = vec![0.0; ndof];
        for &dof in &fixed {
            reactions[dof] = residual[dof];
        }

        let mut element_ids = Vec::with_capacity(members.len());
        let mut member_forces = vec![0.0; members.len() * FORCES_PER_MEMBER];
        for (position, member) in members.iter().enumerate() {
            let mut local_displacements = DVector::<f64>::zeros(12);
            for row in 0..12 {
                local_displacements[row] = displacements[member.dofs[row]];
            }
            let end_forces = &member.k_local * (&member.t * &local_displacements);
            let fixed_contribution = match fixed_end.get(&member.element_id) {
                Some(load) => load.clone(),
                None => DVector::<f64>::zeros(12),
            };
            element_ids.push(member.element_id);
            for row in 0..FORCES_PER_MEMBER {
                member_forces[position * FORCES_PER_MEMBER + row] = end_forces[row] - fixed_contribution[row];
            }
        }

        let peak = displacements.iter().fold(0.0_f64, |max, value| max.max(value.abs()));
        AnalysisResults {
            status: format!(
                "{status} | {} joint(s), {} member(s), max |d| = {peak:.3e} m",
                ids.len(),
                members.len()
            ),
            node_ids: ids,
            displacements: displacements.iter().copied().collect(),
            reactions,
            element_ids,
            member_forces,
        }
    }

    /// Solve the first load case (what the tables show before a combination is chosen).
    pub fn run_static(&self) -> AnalysisResults {
        match self.load_cases.first() {
            Some(case) => self.run_case(case),
            None => AnalysisResults::default(),
        }
    }
    /// Evaluate every stored load combination over the current load cases.
    pub fn run_combinations(&self) -> Vec<CombinationResult> {
        let mut evaluated = Vec::new();
        // Each case is solved once and reused by every combination that names it;
        // re-solving per term made a 33-combination model take minutes.
        let mut cache: HashMap<usize, AnalysisResults> = HashMap::new();
        for combo in &self.model.load_combinations {
            let solved: Vec<(f64, AnalysisResults)> = combo
                .factors
                .iter()
                .filter_map(|(case_id, factor)| {
                    let case = self.load_cases.iter().find(|case| case.id == *case_id)?;
                    let result = cache.entry(*case_id).or_insert_with(|| self.run_case(case));
                    Some((*factor, result.clone()))
                })
                .collect();
            if solved.is_empty() {
                continue;
            }
            let base = &solved[0].1;
            let mut displacements = vec![0.0; base.displacements.len()];
            let mut reactions = vec![0.0; base.reactions.len()];
            let mut forces = vec![0.0; base.member_forces.len()];
            for (factor, result) in &solved {
                combine(&mut displacements, &result.displacements, *factor, &combo.combo_type);
                combine(&mut reactions, &result.reactions, *factor, &combo.combo_type);
                combine(&mut forces, &result.member_forces, *factor, &combo.combo_type);
            }
            if matches!(&combo.combo_type, ComboType::Ssrs | ComboType::Cqc) {
                for value in displacements.iter_mut().chain(reactions.iter_mut()).chain(forces.iter_mut()) {
                    *value = value.sqrt();
                }
            }
            let max_displacement = displacements.iter().fold(0.0_f64, |max, value| max.max(value.abs()));
            let max_reaction = reactions.iter().fold(0.0_f64, |max, value| max.max(value.abs()));
            evaluated.push(CombinationResult {
                name: combo.name.clone(),
                combo_type: combo.combo_type.clone(),
                equation: combo.equation.clone(),
                status: format!("Solved ({} term(s))", solved.len()),
                max_displacement,
                max_reaction,
                results: AnalysisResults {
                    status: format!("{} | {} term(s)", combo.name, solved.len()),
                    node_ids: base.node_ids.clone(),
                    displacements,
                    reactions,
                    element_ids: base.element_ids.clone(),
                    member_forces: forces,
                },
            });
        }
        evaluated
    }

    /// Sum of the support reactions, in global X, Y and Z.
    pub fn reaction_sums(&self, results: &AnalysisResults) -> Vec<f64> {
        let mut sums = vec![0.0; 3];
        for joint in 0..results.node_ids.len() {
            for (axis, slot) in sums.iter_mut().enumerate() {
                *slot += results.reactions.get(joint * DOFS_PER_JOINT + axis).copied().unwrap_or(0.0);
            }
        }
        sums
    }

    /// Largest absolute joint displacement in the result set.
    pub fn max_displacement(&self, results: &AnalysisResults) -> f64 {
        results.displacements.iter().fold(0.0_f64, |max, value| max.max(value.abs()))
    }

    /// Combine modal responses by the complete quadratic combination rule.
    pub fn modal_combination_cqc(
        &self,
        scaled: &[(f64, DVector<f64>)],
        _index: &HashMap<usize, usize>,
        ndof: usize,
    ) -> Result<AnalysisResults, String> {
        let mut displacements = vec![0.0_f64; ndof];
        for (scale, vector) in scaled {
            for (slot, value) in vector.iter().enumerate().take(ndof) {
                displacements[slot] += scale * value;
            }
        }
        Ok(AnalysisResults {
            status: format!("CQC over {} mode(s)", scaled.len()),
            node_ids: self.model.nodes.keys().copied().collect(),
            displacements,
            reactions: vec![0.0; ndof],
            element_ids: self.model.elements.keys().copied().collect(),
            member_forces: vec![0.0; self.model.elements.len() * FORCES_PER_MEMBER],
        })
    }
}

/// Fold one scaled result component set into a combination accumulator.
fn combine(target: &mut [f64], source: &[f64], factor: f64, kind: &ComboType) {
    for (slot, value) in target.iter_mut().zip(source) {
        match kind {
            ComboType::Abs => *slot += (factor * value).abs(),
            ComboType::Ssrs | ComboType::Cqc => *slot += (factor * value).powi(2),
            ComboType::Envelope => {
                let candidate = factor * value;
                if candidate.abs() > slot.abs() {
                    *slot = candidate;
                }
            }
            ComboType::LinearAdd => *slot += factor * value,
        }
    }
}

impl LoadCase {
    /// Convenience wrapper: a case carries no model, so this returns an empty set.
    pub fn run_static(&self) -> AnalysisResults {
        AnalysisResults::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 6 m simply supported beam in two elements under 10 kN/m.
    fn simply_supported() -> (Analysis, usize) {
        let mut model = Model::new("beam");
        let material = model.add_material(crate::materials::Material::steel());
        let section = model.add_section(crate::sections::Section::rectangular("RECT300X500", 0.3, 0.5));
        let node_i = model.add_node(0.0, 0.0, 0.0);
        let node_mid = model.add_node(3.0, 0.0, 0.0);
        let node_j = model.add_node(6.0, 0.0, 0.0);
        let left = model.add_element(Element::frame(1, node_i, node_mid, material, section));
        let right = model.add_element(Element::frame(2, node_mid, node_j, material, section));
        model.nodes.get_mut(&node_i).unwrap().restrained = [true, true, true, true, false, false];
        model.nodes.get_mut(&node_j).unwrap().restrained = [false, true, true, false, false, false];
        for element_id in [left, right] {
            model.frame_loads.push(crate::loads::DistributedLoad {
                element_id,
                direction: LoadDirection::Global,
                start: [0.0, 0.0, -10_000.0],
                end: [0.0, 0.0, -10_000.0],
                projected: false,
                case_id: None,
            });
        }
        let mut analysis = Analysis::new(model);
        analysis.load_cases = vec![LoadCase {
            id: 1,
            name: "CASE 1".into(),
            pattern: LoadType::Dead,
            self_weight_multiplier: 0.0,
            nodal_loads: Vec::new(),
            distributed_loads: Vec::new(),
            p_delta: false,
        }];
        (analysis, node_mid)
    }

    #[test]
    fn vertical_column_carries_axial_force() {
        let mut model = Model::new("column");
        let material = model.add_material(crate::materials::Material::steel());
        let section = model.add_section(crate::sections::Section::rectangular("RECT300X300", 0.3, 0.3));
        let base = model.add_node(0.0, 0.0, 0.0);
        let top = model.add_node(0.0, 0.0, 3.0);
        let column = model.add_element(Element::frame(1, base, top, material, section));
        model.nodes.get_mut(&base).unwrap().restrained = [true; 6];
        let mut analysis = Analysis::new(model);
        analysis.load_cases = vec![LoadCase {
            id: 1,
            name: "CASE 1".into(),
            pattern: LoadType::Dead,
            self_weight_multiplier: 0.0,
            nodal_loads: vec![crate::loads::NodalLoad {
                node_id: top,
                fx: 0.0,
                fy: 0.0,
                fz: -100_000.0,
                mx: 0.0,
                my: 0.0,
                mz: 0.0,
            }],
            distributed_loads: Vec::new(),
            p_delta: false,
        }];
        let results = analysis.run_static();
        assert!(results.status.starts_with("Solved"), "status: {}", results.status);
        let reaction = results.reactions_of(base).expect("base reaction");
        assert!((reaction[2] - 100_000.0).abs() < 1.0, "base Fz = {}", reaction[2]);
        let forces = results.member_forces_of(column).expect("column forces");
        assert!(
            (forces[0].abs() - 100_000.0).abs() < 1.0,
            "column axial = {} (all: {forces:?})",
            forces[0]
        );
    }

    #[test]
    fn area_pressure_reaches_the_perimeter_frame() {
        let mut model = Model::new("slab");
        let material = model.add_material(crate::materials::Material::steel());
        let section = model.add_section(crate::sections::Section::rectangular("RECT300X500", 0.3, 0.5));
        let corners = [
            model.add_node(0.0, 0.0, 0.0),
            model.add_node(4.0, 0.0, 0.0),
            model.add_node(4.0, 3.0, 0.0),
            model.add_node(0.0, 3.0, 0.0),
        ];
        for index in 0..4 {
            model.add_element(Element::frame(
                index + 1,
                corners[index],
                corners[(index + 1) % 4],
                material,
                section,
            ));
            model.nodes.get_mut(&corners[index]).unwrap().restrained = [true; 6];
        }
        let area = model.add_area(crate::areas::Area::quadrilateral(
            corners[0], corners[1], corners[2], corners[3], material, 0.15,
        ));
        model.area_loads.push(crate::loads::AreaLoad {
            area_id: area,
            direction: LoadDirection::Global,
            pressure: 2_000.0,
            case_id: Some(1),
        });
        let mut analysis = Analysis::new(model);
        analysis.load_cases = vec![LoadCase {
            id: 1,
            name: "CASE 1".into(),
            pattern: LoadType::Dead,
            self_weight_multiplier: 0.0,
            nodal_loads: Vec::new(),
            distributed_loads: Vec::new(),
            p_delta: false,
        }];
        let results = analysis.run_static();
        assert!(results.status.starts_with("Solved"), "status: {}", results.status);
        let sums = analysis.reaction_sums(&results);
        assert!((sums[2] - 24_000.0).abs() < 1.0, "sum Fz = {}", sums[2]);
    }

    #[test]
    fn simply_supported_beam_reactions_balance() {
        let (analysis, _) = simply_supported();
        let results = analysis.run_static();
        assert!(results.status.starts_with("Solved"), "status: {}", results.status);
        let sums = analysis.reaction_sums(&results);
        assert!((sums[2] - 60_000.0).abs() < 1.0, "sum Fz = {}", sums[2]);
        assert!(sums[0].abs() < 1.0, "sum Fx = {}", sums[0]);
        assert!(sums[1].abs() < 1.0, "sum Fy = {}", sums[1]);
    }

    #[test]
    fn simply_supported_beam_deflection_matches_theory() {
        let (analysis, node_mid) = simply_supported();
        let results = analysis.run_static();
        let deflection = results.displacement_of(node_mid).expect("midspan")[2].abs();
        let section = analysis.model.sections.values().next().expect("section");
        let modulus = analysis.model.materials.values().next().expect("material").e;
        let analytic = 5.0 * 10_000.0 * 6.0_f64.powi(4) / (384.0 * modulus * section.iy);
        assert!(
            (deflection - analytic).abs() / analytic < 0.02,
            "deflection {deflection:e} vs analytic {analytic:e}"
        );
    }

    #[test]
    fn linear_add_combination_scales_the_case() {
        let (mut analysis, node_mid) = simply_supported();
        let single = analysis.run_static().displacement_of(node_mid).expect("midspan")[2];
        analysis.model.load_combinations.push(crate::loads::LoadCombination {
            id: 1,
            name: "2CASE1".into(),
            equation: "2*CASE 1".into(),
            combo_type: ComboType::LinearAdd,
            factors: vec![(1, 2.0)],
        });
        let combos = analysis.run_combinations();
        assert_eq!(combos.len(), 1);
        let doubled = combos[0].results.displacement_of(node_mid).expect("midspan")[2];
        assert!((doubled - 2.0 * single).abs() < 1.0e-9 * single.abs().max(1.0e-12));
    }
}
