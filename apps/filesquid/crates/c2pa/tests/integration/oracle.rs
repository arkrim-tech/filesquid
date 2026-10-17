//! C2PA-S-007: interoperability oracle run. For every C2PA conformance file it
//! runs `c2patool`, records the oracle verdict and compares it with the
//! engine's report. Any disagreement fails the test; files the engine cannot
//! read yet are counted as pending (manifest reading lands in CP-0.2 to
//! CP-0.5). Skipped, with the reason printed, when `c2patool` or the corpus is
//! not available (see the crate README, "Interoperability oracle").

#[path = "../support/oracle.rs"]
mod oracle;

use std::collections::BTreeMap;
use std::fs;

use oracle::{
    compare, corpus_files, engine_report, oracle_setup_from_env, run_c2patool, Agreement,
    OracleConfig, OracleSetup, MAX_CORPUS_FILES,
};

/// Counts of one oracle run.
#[derive(Debug, Default)]
struct Tally {
    /// Files where engine and oracle agree.
    agree: usize,
    /// Files the engine cannot read yet.
    pending: usize,
    /// Oracle verdicts by name, for the log.
    verdicts: BTreeMap<String, usize>,
    /// Disagreements, one line each.
    disagreements: Vec<String>,
}

/// Run the oracle and the engine on every corpus file.
fn run_corpus(config: &OracleConfig) -> Tally {
    let files = corpus_files(&config.corpus, MAX_CORPUS_FILES).expect("corpus walk");
    assert!(
        !files.is_empty(),
        "no assets under {}",
        config.corpus.display()
    );
    let mut tally = Tally::default();
    for file in &files {
        let report = run_c2patool(&config.tool, file).expect("c2patool report");
        *tally
            .verdicts
            .entry(format!("{:?}", report.verdict))
            .or_default() += 1;
        let bytes = fs::read(file).expect("asset bytes");
        match compare(&report, engine_report(&bytes).as_ref()) {
            Agreement::Agree => tally.agree += 1,
            Agreement::EnginePending => tally.pending += 1,
            Agreement::Disagree { oracle, engine } => tally.disagreements.push(format!(
                "{}: oracle {oracle:?}, engine {engine:?}",
                file.display()
            )),
        }
    }
    tally
}

#[test]
fn engine_agrees_with_c2patool_on_the_conformance_files() {
    let config = match oracle_setup_from_env() {
        OracleSetup::Ready(config) => config,
        OracleSetup::Skipped(reason) => {
            eprintln!("SKIPPED c2patool oracle: {reason}");
            return;
        }
    };
    let tally = run_corpus(&config);
    eprintln!(
        "c2patool oracle: {} agree, {} pending engine reader, {} disagree; oracle verdicts {:?}",
        tally.agree,
        tally.pending,
        tally.disagreements.len(),
        tally.verdicts
    );
    assert!(
        tally.disagreements.is_empty(),
        "engine disagrees with c2patool:\n{}",
        tally.disagreements.join("\n")
    );
}
