#![allow(non_snake_case)]
use crate::errors::{EigenError, Result};
use crate::expressionops::ExpressionOps as E;
use crate::starkinfo_codegen::{
    build_code, iterate_code, pil_code_gen, Context, ContextF, EVIdx, Index, IndexVec, Node,
    PolType, Segment,
};
use crate::types::{Expression, Public, StarkStruct, PIL};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;

#[derive(Default, Debug, Serialize)]
pub struct PCCTX {
    pub f_exp_id: usize,
    pub t_exp_id: usize,
    pub h1_id: usize,
    pub h2_id: usize,
    pub z_id: usize,
    pub c1_id: usize,
    pub c2_id: usize,
    pub num_id: usize,
    pub den_id: usize,
}

#[derive(Debug, Default)]
pub struct Program {
    pub publics_code: Vec<Segment>,
    pub step2prev: Segment,
    pub step3prev: Segment,
    pub step3: Segment,
    pub step42ns: Segment,
    pub step52ns: Segment,
    pub verifier_code: Segment,
    pub verifier_query_code: Segment,
}

#[derive(Debug, Default, Serialize)]
pub struct StarkInfo {
    pub var_pol_map: Vec<PolType>,
    pub n_cm1: usize,
    pub n_cm2: usize,
    pub n_cm3: usize,
    pub n_cm4: usize,
    pub n_q: usize,
    pub pu_ctx: Vec<PCCTX>,
    pub pe_ctx: Vec<PCCTX>,
    pub ci_ctx: Vec<PCCTX>,
    pub n_constants: usize,
    pub n_publics: usize,
    pub c_exp: usize,

    pub im_exps: HashMap<usize, bool>,
    pub q_deg: usize,
    pub q_dim: usize,
    pub im_exps_list: Vec<usize>,
    pub im_exp2cm: HashMap<usize, usize>,

    pub qs: Vec<usize>,
    pub exps_2ns: Vec<usize>,
    pub exps_n: Vec<usize>,

    pub ev_map: Vec<Node>,
    pub fri_exp_id: usize,
    pub n_exps: usize,

    pub cm_n: Vec<usize>,
    pub cm_2ns: Vec<usize>,
    pub tmpexp_n: Vec<usize>,
    pub q_2ns: Vec<usize>,
    pub f_2ns: Vec<usize>,

    pub map_sections: IndexVec,
    pub map_sectionsN1: Index,
    pub map_sectionsN3: Index,
    pub map_sectionsN: Index,
    pub map_offsets: Index,
    pub map_deg: Index,
    pub map_total_n: usize,
    pub exp2pol: HashMap<usize, usize>,

    pub publics: Vec<Public>,
    pub ev_idx: EVIdx,
}

impl fmt::Display for StarkInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let obj = json!(self);
        write!(f, "{}", serde_json::to_string_pretty(&obj).unwrap())
    }
}

impl StarkInfo {
    pub fn new(pil: &mut PIL, stark_struct: &StarkStruct) -> Result<(StarkInfo, Program)> {
        let pil_deg = pil.references.values().nth(0).unwrap().polDeg;

        let stark_deg = 2usize.pow(stark_struct.nBits as u32);

        if stark_deg != pil_deg {
            return Err(EigenError::MustEqualDegreeError(stark_deg, pil_deg));
        }

        if stark_struct.nBitsExt != stark_struct.steps[0].nBits {
            return Err(EigenError::MustEqualDegreeError(
                stark_struct.nBitsExt,
                stark_struct.steps[0].nBits,
            ));
        }

        let mut info = StarkInfo {
            var_pol_map: Vec::new(),
            pu_ctx: Vec::new(),
            pe_ctx: Vec::new(),
            ci_ctx: Vec::new(),
            n_constants: pil.nConstants,
            n_publics: pil.publics.len(),
            exp2pol: HashMap::new(),
            n_cm1: 0,
            n_cm2: 0,
            n_cm3: 0,
            n_cm4: 0,
            n_q: 0,
            c_exp: 0,
            ev_map: Vec::new(),
            fri_exp_id: 0,
            n_exps: 0,
            q_deg: 0,
            q_dim: 0,
            im_exps: HashMap::new(),
            im_exps_list: Vec::new(),
            im_exp2cm: HashMap::new(),
            qs: Vec::new(),
            exps_2ns: Vec::new(),
            exps_n: Vec::new(),
            cm_n: Vec::new(),
            cm_2ns: Vec::new(),
            tmpexp_n: Vec::new(),
            q_2ns: Vec::new(),
            f_2ns: Vec::new(),
            map_sections: IndexVec::default(),
            map_sectionsN1: Index::default(),
            map_sectionsN3: Index::default(),
            map_sectionsN: Index::default(),
            map_offsets: Index::default(),
            map_deg: Index::default(),
            map_total_n: 0,
            publics: Vec::new(),
            ev_idx: EVIdx::new(),
        };

        let mut program = Program {
            publics_code: vec![],
            step2prev: Segment::default(),
            step3prev: Segment::default(),
            step3: Segment::default(),
            step42ns: Segment::default(),
            step52ns: Segment::default(),
            verifier_code: Segment::default(),
            verifier_query_code: Segment::default(),
        };

        info.generate_pubulic_calculators(pil, &mut program)?;
        info.n_cm1 = pil.nCommitments;

        let mut ctx = Context {
            tmp_used: 0,
            code: vec![],
            calculated: HashMap::new(),
            exp_id: 0,
        };

        let mut ctx2ns = Context {
            tmp_used: 0,
            code: vec![],
            calculated: HashMap::new(),
            exp_id: 0,
        };

        log::info!("generate_step2");
        info.generate_step2(&mut ctx, pil, &mut program)?; // H1, H2

        log::info!("generate_step3");
        info.generate_step3(&mut ctx, pil, &mut program)?; // Z Polynonmial and LC of the permutation checks

        log::info!("generate_constraint_polynomial");
        info.generate_constraint_polynomial(
            &mut ctx,
            &mut ctx2ns,
            pil,
            stark_struct,
            &mut program,
        )?;

        let mut ctx = Context {
            tmp_used: 0,
            code: vec![],
            calculated: HashMap::new(),
            exp_id: 0,
        };
        for (k, v) in info.im_exps.iter() {
            ctx.calculated.insert(("exps", *k), *v);
            ctx.calculated.insert(("expsPrime", *k), *v);
        }

        log::info!("generate_constraint_polynomial_verifier");
        info.generate_constraint_polynomial_verifier(&mut ctx, pil, &mut program)?;
        log::info!("generate_fri_polynomial");
        info.generate_fri_polynomial(&mut ctx2ns, pil, &mut program)?;

        let mut ctx = Context {
            tmp_used: 0,
            code: vec![],
            calculated: HashMap::new(),
            exp_id: 0,
        };
        log::info!("generate_fri_verifier");
        info.generate_fri_verifier(&mut ctx, pil, &mut program)?;

        log::info!("map");
        info.map(pil, &stark_struct, &mut program)?;

        info.publics = pil.publics.clone();
        Ok((info, program))
    }

    pub fn generate_pubulic_calculators(
        &mut self,
        pil: &mut PIL,
        program: &mut Program,
    ) -> Result<()> {
        let publics = pil.publics.clone();
        log::debug!(
            "generate_pubulic_calculators: publics as input: {:?}",
            publics
        );
        for p in publics.iter() {
            if p.polType.as_str() == "imP" {
                let mut ctx = Context {
                    tmp_used: 0,
                    code: vec![],
                    calculated: HashMap::new(),
                    exp_id: 0,
                };
                pil_code_gen(&mut ctx, pil, p.polId, false, "", 0)?;
                let mut segment = build_code(&mut ctx, pil);

                let mut ctx_f = ContextF {
                    exp_map: HashMap::new(),
                    tmp_used: segment.tmp_used,
                    dom: "".to_string(),
                    tmpexps: &mut HashMap::new(),
                    starkinfo: self,
                };

                let fix_ref = |r: &mut Node, ctx: &mut ContextF, _pil: &mut PIL| {
                    let p = if r.prime { 1 } else { 0 };
                    if r.type_.as_str() == "exp" {
                        if ctx.exp_map.get(&(p, r.id)).is_none() {
                            ctx.exp_map.insert((p, r.id), ctx.tmp_used);
                            ctx.tmp_used += 1;
                        }
                        r.prime = false;
                        r.type_ = "tmp".to_string();
                        r.id = *ctx.exp_map.get(&(p, r.id)).unwrap();
                    }
                };
                iterate_code(&mut segment, fix_ref, &mut ctx_f, pil);

                segment.tmp_used = ctx_f.tmp_used;
                program.publics_code.push(segment);
                //log::debug!("generate_pubulic_calculators: publics_code: {}", program.publics_code.len());
                //    log::debug!("{}", pp);
                //}
                ctx.calculated.clear(); // TODO: useless
            }
        }
        Ok(())
    }

    pub fn generate_step2(
        &mut self,
        ctx: &mut Context,
        pil: &mut PIL,
        program: &mut Program,
    ) -> Result<()> {
        let ppi = pil.plookupIdentities.clone();
        log::debug!("generate_step2: [{:?}]", ppi);
        for pi in ppi.iter() {
            let u = E::challenge("u".to_string());
            let def_val = E::challenge("defVal".to_string());

            let mut t_exp: Expression = E::nop();
            for j in pi.t.as_ref().unwrap().iter() {
                let e = E::exp(*j, None);
                if E::is_nop(&t_exp) {
                    t_exp = e;
                } else {
                    t_exp = E::add(&E::mul(&u, &t_exp), &e);
                }
            }

            if pi.selT.is_some() {
                t_exp = E::sub(&t_exp, &def_val);
                t_exp = E::mul(&t_exp, &E::exp(pi.selT.unwrap(), None));
                t_exp = E::add(&t_exp, &def_val);
                t_exp.idQ = Some(pil.nQ);
                pil.nQ += 1;
            }

            let t_exp_id = pil.expressions.len();
            t_exp.keep = Some(true);
            pil.expressions.push(t_exp);

            let mut f_exp = E::nop();
            for j in pi.f.as_ref().unwrap().iter() {
                let e = E::exp(j.clone(), None);
                if f_exp == E::nop() {
                    f_exp = e;
                } else {
                    f_exp = E::add(&E::mul(&f_exp, &u), &e);
                }
            }
            if pi.selF.is_some() {
                f_exp = E::sub(&f_exp, &E::exp(t_exp_id, None));
                f_exp = E::mul(&f_exp, &E::exp(pi.selF.unwrap(), None));
                f_exp = E::add(&f_exp, &E::exp(t_exp_id, None));

                f_exp.idQ = Some(pil.nQ);
                pil.nQ += 1;
            }

            let f_exp_id = pil.expressions.len();
            f_exp.keep = Some(true);
            pil.expressions.push(f_exp);

            pil_code_gen(ctx, pil, f_exp_id, false, "", 0)?;
            pil_code_gen(ctx, pil, t_exp_id, false, "", 0)?;

            let h1_id = pil.nCommitments;
            pil.nCommitments += 1;
            let h2_id = pil.nCommitments;
            pil.nCommitments += 1;

            self.pu_ctx.push(PCCTX {
                f_exp_id,
                t_exp_id,
                h1_id,
                h2_id,
                z_id: 0,
                c1_id: 0,
                c2_id: 0,
                num_id: 0,
                den_id: 0,
            });
        }

        program.step2prev = build_code(ctx, pil);
        log::debug!("pu_ctx {:?}", self.pu_ctx);
        log::debug!("step2prev {}", program.step2prev);
        ctx.calculated.clear();
        self.n_cm2 = pil.nCommitments - self.n_cm1;
        log::debug!("n_cm2 {}", self.n_cm2);
        Ok(())
    }
}
