#![cfg(feature = "test-support")]
#![allow(clippy::unwrap_used)]

use codeconvoy::{agents, domain::AgentId};
use std::path::Path;

#[tokio::test]
async fn discovered_candidates_use_each_backends_existing_compatibility_check_without_a_task() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_BIN_EXE_codeconvoy-test-agent"));
    for agent in AgentId::ALL {
        let candidates = agents::discovery::find(agent, fixture.to_str().unwrap());
        assert_eq!(candidates.first().map(|path| path.as_path()), Some(fixture));
        let options = [("executable".into(), candidates[0].display().to_string())].into();
        let backend = agents::backend(agent).unwrap();
        let result = agents::detect(backend.as_ref(), &options, directory.path())
            .await
            .unwrap();
        assert!(result.contains("CLI compatibility checked"));
        assert!(!directory.path().join("agent-input").exists());
    }
}
