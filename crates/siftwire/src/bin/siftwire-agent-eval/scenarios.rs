use crate::types::{Scenario, Turn};

pub const RUNNER_ONLY_INSTRUCTION: &str = "Use the siftwire project skill. Read only .agents/skills/siftwire/SKILL.md directly, then use only the siftwire runner JSON interface. Do not inspect other repo files, source files, binaries, SQLite, environment variables, or run siftwire --help. Do not search for instructions. This host eval uses synthetic local fixtures and simulates successful transport of each prepared message/text/html unchanged; no real email or external delivery occurs. After prepare_delivery succeeds, treat that simulated transport as successful and call confirm_delivery. Never send email or call an external transport. Require siftwire-runner/v5, prepared-delivery/v1, and current-news/v1; there is no manual delivery fallback.";

pub fn all() -> Vec<Scenario> {
    let mut values = first_group();
    values.extend(second_group());
    values.extend(third_group());
    values
}

fn entry(id: &'static str, prompts: &[&str]) -> Scenario {
    Scenario {
        id,
        turns: prompts
            .iter()
            .map(|prompt| Turn {
                prompt: (*prompt).to_owned(),
            })
            .collect(),
    }
}

fn first_group() -> Vec<Scenario> {
    vec![
        entry(
            "empty-config-rejects-run-brief",
            &[
                "Run a SiftWire brief from a fresh empty configuration and report the production runner result.",
            ],
        ),
        entry(
            "rss-source-first-run-candidate",
            &[
                "Configure an RSS source for https://github.blog/feed/ with key github-blog, section technology, and threshold medium. Then run a SiftWire brief and report the JSON-derived brief.",
            ],
        ),
        entry(
            "github-release-source-config",
            &[
                "Configure a GitHub release source for repository openai/codex with key codex-releases, section releases, and threshold always. Inspect configuration and report the repository-derived source. Do not run a brief or fetch releases in this offline configuration scenario.",
            ],
        ),
        entry(
            "rss-source-must-include",
            &[
                "Configure an RSS source for https://github.blog/feed/ with key required-feed, section technology, and threshold always. Run a brief, prepare delivery with no optional candidates, confirm after simulated transport, and report final_answer.",
            ],
        ),
        entry(
            "repeat-run-no-new-items",
            &[
                "Configure an RSS source for https://github.blog/feed/ with key github-blog, section technology, and threshold medium. Run a SiftWire brief, prepare delivery, confirm after simulated transport, and report final_answer.",
                "Run SiftWire again without changing configuration and report the production runner result.",
            ],
        ),
    ]
}

fn second_group() -> Vec<Scenario> {
    vec![
        entry(
            "rss-source-generic-processing-fields",
            &[
                "Configure an RSS source for https://github.blog/feed/ with key github-blog, section technology, threshold medium, url_canonicalization none, outlet_extraction title_suffix, dedup_group news, and priority_rank 10. Then run a SiftWire brief with run_brief, prepare delivery, confirm after simulated transport, and report final_answer.",
            ],
        ),
        entry(
            "outlet-policy-watch-audit",
            &[
                "Configure an outlet policy named Fixture Outlet with policy watch and enabled true. Configure an RSS source for https://github.blog/feed/ with key github-blog, section technology, threshold medium, and outlet_extraction title_suffix. Run SiftWire and report whether the JSON result includes a policy audit while still allowing candidates.",
            ],
        ),
        entry(
            "configured-max-delivery-items",
            &[
                "Configure SiftWire max_delivery_items to 2 through siftwire config. Configure RSS sources with keys limit-one, limit-two, and limit-three for https://example.com/siftwire-limit-1.xml, https://example.com/siftwire-limit-2.xml, and https://example.com/siftwire-limit-3.xml, each with section technology and threshold medium. Run a SiftWire brief, prepare delivery with exactly two returned candidate indexes, confirm after simulated transport, and report only final_answer.",
            ],
        ),
        entry(
            "brief-run-history",
            &[
                "Configure exactly one RSS source by piping {\"action\":\"upsert_source\",\"source\":{\"key\":\"history\",\"label\":\"History\",\"kind\":\"rss\",\"url\":\"https://example.com/siftwire-history-1.xml\",\"section\":\"technology\",\"threshold\":\"medium\",\"enabled\":true}} to siftwire config. Then pipe {\"action\":\"run_brief\",\"dry_run\":false} to siftwire brief. Prepare delivery selecting the returned candidate index, confirm after simulated transport, and report final_answer unchanged.",
                "Replace the history source URL by piping {\"action\":\"upsert_source\",\"source\":{\"key\":\"history\",\"label\":\"History\",\"kind\":\"rss\",\"url\":\"https://example.com/siftwire-history-2.xml\",\"section\":\"technology\",\"threshold\":\"medium\",\"enabled\":true}} to siftwire config. Then run SiftWire, prepare delivery selecting the returned candidate index, confirm after simulated transport, and report final_answer unchanged.",
                "Replace the history source URL by piping {\"action\":\"upsert_source\",\"source\":{\"key\":\"history\",\"label\":\"History\",\"kind\":\"rss\",\"url\":\"https://example.com/siftwire-history-3.xml\",\"section\":\"technology\",\"threshold\":\"medium\",\"enabled\":true}} to siftwire config. Then run SiftWire, prepare delivery selecting the returned candidate index, confirm after simulated transport, and report final_answer unchanged with Current brief and Previous brief sections.",
            ],
        ),
    ]
}

fn third_group() -> Vec<Scenario> {
    vec![
        entry(
            "feed-failure-health-footnote",
            &[
                "Configure an RSS source with key broken-feed, label Broken Feed, URL https://example.com/siftwire-missing.xml, section technology, and threshold medium. Run a SiftWire brief and report the health footnote from the JSON result.",
            ],
        ),
        entry(
            "feed-recovery-resolves-warning",
            &[
                "Configure an RSS source with key changing-feed, label Changing Feed, URL https://example.com/siftwire-missing.xml, section technology, and threshold medium. Run a SiftWire brief and report the JSON result.",
                "Replace the changing-feed source URL with https://github.blog/feed/. Run SiftWire again and report the JSON-derived result.",
            ],
        ),
        entry(
            "invalid-source-config-rejects",
            &[
                "Try to configure a SiftWire source with an invalid key Bad/Key by piping one upsert_source JSON request to siftwire config. Report the production runner rejection.",
            ],
        ),
        entry(
            "routine-agent-hygiene",
            &[
                "Run a normal SiftWire configuration inspection by piping exactly {\"action\":\"inspect_config\"} to siftwire config.",
            ],
        ),
    ]
}
