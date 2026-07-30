use anyhow::{Result, bail};
use karukan_im::core::engine::thread_budget_bench::{
    run_thread_budget_benchmark, run_thread_budget_operation_benchmark,
};

fn usage() -> &'static str {
    "usage: inference-thread-budget-bench [--runs N] --load-mode idle|10yes [--operations] [--output PATH]"
}

fn main() -> Result<()> {
    let mut runs = 30usize;
    let mut load_mode = None;
    let mut output = None;
    let mut operations = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--runs" => {
                runs = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!(usage()))?
                    .parse()
                    .map_err(|_| anyhow::anyhow!(usage()))?;
            }
            "--load-mode" => {
                load_mode = Some(args.next().ok_or_else(|| anyhow::anyhow!(usage()))?);
            }
            "--output" => {
                output = Some(args.next().ok_or_else(|| anyhow::anyhow!(usage()))?);
            }
            "--operations" => operations = true,
            _ => bail!(usage()),
        }
    }
    let load_mode = load_mode.ok_or_else(|| anyhow::anyhow!(usage()))?;
    if !matches!(load_mode.as_str(), "idle" | "10yes") {
        bail!(usage());
    }

    let (json, candidate_quality_gate_passed) = if operations {
        let report = run_thread_budget_operation_benchmark(runs, load_mode)?;
        (
            serde_json::to_string_pretty(&report)?,
            report.candidate_quality_gate_passed,
        )
    } else {
        let report = run_thread_budget_benchmark(runs, load_mode)?;
        (
            serde_json::to_string_pretty(&report)?,
            report.candidate_quality_gate_passed,
        )
    };
    if let Some(path) = output {
        std::fs::write(path, json)?;
    } else {
        println!("{json}");
    }
    if !candidate_quality_gate_passed {
        bail!("P4 candidate quality gate failed")
    }
    Ok(())
}
