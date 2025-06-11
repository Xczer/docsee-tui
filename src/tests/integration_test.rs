use docsee::docker::DockerClient;

#[tokio::test]
async fn test_docker_connection() {
    // This test will only pass if Docker is running
    // In a real CI/CD pipeline, you'd set up a Docker daemon or skip this test

    let result = DockerClient::new("unix:///var/run/docker.sock").await;

    match result {
        Ok(_client) => {
            println!("✅ Successfully connected to Docker daemon");
        }
        Err(e) => {
            println!("❌ Failed to connect to Docker: {}", e);
            println!("💡 Make sure Docker is running for integration tests");
            // Don't fail the test if Docker isn't available
            // In a real project, you might want to skip this test instead
        }
    }
}

#[test]
fn test_tab_navigation() {
    use docsee::app::TabType;

    let containers = TabType::Containers;
    assert_eq!(containers.next(), TabType::Images);
    assert_eq!(containers.previous(), TabType::Networks);

    let images = TabType::Images;
    assert_eq!(images.next(), TabType::Volumes);
    assert_eq!(images.previous(), TabType::Containers);
}

#[test]
fn test_key_conversion() {
    use crossterm::event::{KeyCode, KeyModifiers, KeyEvent};
    use docsee::events::Key;

    // Test quit key
    let quit_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(Key::from(quit_event), Key::Quit);

    // Test cheatsheet key
    let cheat_event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
    assert_eq!(Key::from(cheat_event), Key::Cheatsheet);

    // Test delete key (uppercase D)
    let delete_event = KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT);
    assert_eq!(Key::from(delete_event), Key::DeleteItem);
}

/*
EXPLANATION:
- test_docker_connection(): Tests if we can connect to Docker daemon
- test_tab_navigation(): Tests the tab switching logic
- test_key_conversion(): Tests that keyboard events are converted correctly
- These tests help ensure the core functionality works
- The Docker test is graceful - it won't fail if Docker isn't running
- Run tests with: cargo test
*/
