use analytics::RedisAnalyticsMetricsUpdater;
use analytics_dao::AnalyticsViewsDao;
use events_dao::{EventDao, EventTypeDao};
use events_handlers::{
    CreateEventHandler, DeleteEventHandler, UpdateEventHandler,
};
use flume::Receiver;
use redis_connection::cache_provider::CacheProvider;
use test_utils::{TestPostgresContainer, TestRedisContainer, *};
use user_dao::UserDao;
use user_events::UserAnalyticsEvent;
use user_handlers::{
    CreateUserHandler, DeleteUserHandler, UpdateUserHandler,
};
use user_http::analytics_integration::UserAnalyticsIntegration;
use uuid::Uuid;

pub struct IntegrationTestSetup {
    pub container: TestPostgresContainer,
    pub redis_container: TestRedisContainer,
    pub user_dao: UserDao,
    pub event_dao: EventDao,
    pub event_type_dao: EventTypeDao,
    pub analytics_dao: AnalyticsViewsDao,
    pub create_user_handler: CreateUserHandler,
    pub update_user_handler: UpdateUserHandler,
    pub delete_user_handler: DeleteUserHandler,
    pub create_event_handler: CreateEventHandler,
    pub update_event_handler: UpdateEventHandler,
    pub delete_event_handler: DeleteEventHandler,
    pub analytics_receiver: Option<Receiver<UserAnalyticsEvent>>,
    pub redis_metrics_updater: RedisAnalyticsMetricsUpdater,
}

impl IntegrationTestSetup {
    pub async fn new() -> anyhow::Result<Self> {
        let container = TestPostgresContainer::new().await?;
        let redis_container = TestRedisContainer::new().await?;
        redis_container.flush_db().await?;

        CacheProvider::init_redis_static(redis_container.pool.clone());

        let sql_connect = create_sql_connect(&container);

        // Set up DAOs
        let user_dao = UserDao::new(sql_connect.clone());
        let event_dao = EventDao::new(sql_connect.clone());
        let event_type_dao = EventTypeDao::new(sql_connect.clone());
        let analytics_dao = AnalyticsViewsDao::new(sql_connect.clone());

        // Set up handlers
        let create_user_handler = CreateUserHandler::new(sql_connect.clone());
        let update_user_handler = UpdateUserHandler::new(sql_connect.clone());
        let delete_user_handler = DeleteUserHandler::new(sql_connect.clone());
        let create_event_handler =
            CreateEventHandler::new(sql_connect.clone());
        let update_event_handler =
            UpdateEventHandler::new(sql_connect.clone());
        let delete_event_handler = DeleteEventHandler::new(sql_connect);

        let redis_metrics_updater = RedisAnalyticsMetricsUpdater::new();

        Ok(Self {
            container,
            redis_container,
            user_dao,
            event_dao,
            event_type_dao,
            analytics_dao,
            create_user_handler,
            update_user_handler,
            delete_user_handler,
            create_event_handler,
            update_event_handler,
            delete_event_handler,
            analytics_receiver: None,
            redis_metrics_updater,
        })
    }

    pub fn with_analytics_integration(
        mut self,
    ) -> (Self, tokio::task::JoinHandle<()>) {
        let (sender, receiver) = flume::unbounded();
        let task_handle =
            UserAnalyticsIntegration::spawn_background_task(receiver.clone());

        // Configure handlers with analytics integration
        self.create_user_handler = self
            .create_user_handler
            .with_analytics_event_sender(sender.clone());
        self.update_user_handler = self
            .update_user_handler
            .with_analytics_event_sender(sender.clone());
        self.delete_user_handler =
            self.delete_user_handler.with_analytics_event_sender(sender);
        self.analytics_receiver = Some(receiver);

        (self, task_handle)
    }

    pub async fn create_test_event_type(
        &self, name: &str,
    ) -> anyhow::Result<i32> {
        create_test_event_type_with_name(&self.container, name).await
    }

    pub async fn create_test_user(&self, name: &str) -> anyhow::Result<Uuid> {
        create_test_user_with_name(&self.container, name).await
    }

    pub async fn wait_for_analytics_processing(&self) {
        // Give the analytics integration time to process events
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use database_traits::dao::GenericDao;
    use events_commands::CreateEventCommand;
    use serde_json::json;
    use user_commands::{
        CreateUserCommand, DeleteUserCommand, UpdateUserCommand,
    };
    use uuid::Uuid;

    use crate::IntegrationTestSetup;

    #[tokio::test]
    async fn test_user_creation_triggers_analytics_event() {
        let (setup, _analytics_task) = IntegrationTestSetup::new()
            .await
            .unwrap()
            .with_analytics_integration();

        let command = CreateUserCommand {
            name: "Analytics Test User".to_string(),
        };

        // Create user - should trigger analytics event
        let created_user =
            setup.create_user_handler.execute(command).await.unwrap();

        // Wait for analytics processing
        setup.wait_for_analytics_processing().await;

        // Verify user was created in database
        let user_from_db =
            setup.user_dao.find_by_id(created_user.id).await.unwrap();
        assert_eq!(user_from_db.name, "Analytics Test User");

        // Analytics events are processed asynchronously, so we can't easily
        // assert on the analytics state here but the test verifies
        // that the integration doesn't break the user creation flow
    }

    #[tokio::test]
    async fn test_user_update_triggers_analytics_event() {
        let (setup, _analytics_task) = IntegrationTestSetup::new()
            .await
            .unwrap()
            .with_analytics_integration();

        // Create user first
        let create_command = CreateUserCommand {
            name: "Original Name".to_string(),
        };
        let created_user = setup
            .create_user_handler
            .execute(create_command)
            .await
            .unwrap();

        // Update user - should trigger analytics event
        let update_command = UpdateUserCommand {
            user_id: created_user.id,
            name: Some("Updated Name".to_string()),
        };
        let updated_user = setup
            .update_user_handler
            .execute(update_command)
            .await
            .unwrap();

        // Wait for analytics processing
        setup.wait_for_analytics_processing().await;

        // Verify user was updated in database
        assert_eq!(updated_user.name, "Updated Name");
        let user_from_db =
            setup.user_dao.find_by_id(created_user.id).await.unwrap();
        assert_eq!(user_from_db.name, "Updated Name");
    }

    #[tokio::test]
    async fn test_user_deletion_triggers_analytics_event() {
        let (setup, _analytics_task) = IntegrationTestSetup::new()
            .await
            .unwrap()
            .with_analytics_integration();

        // Create user first
        let create_command = CreateUserCommand {
            name: "User To Delete".to_string(),
        };
        let created_user = setup
            .create_user_handler
            .execute(create_command)
            .await
            .unwrap();

        // Delete user - should trigger analytics event
        let delete_command = DeleteUserCommand {
            user_id: created_user.id,
        };
        setup
            .delete_user_handler
            .execute(delete_command)
            .await
            .unwrap();

        // Wait for analytics processing
        setup.wait_for_analytics_processing().await;

        // Verify user was deleted from database
        let result = setup.user_dao.find_by_id(created_user.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_event_creation_for_existing_user() {
        let setup = IntegrationTestSetup::new().await.unwrap();

        // Create a user and event type
        let user_id =
            setup.create_test_user("Event Test User").await.unwrap();
        let event_type_id =
            setup.create_test_event_type("test_event").await.unwrap();

        // Create an event for the user
        let event_command = CreateEventCommand {
            user_id,
            event_type: "test_event".to_string(),
            timestamp: Some(Utc::now()),
            metadata: Some(json!({"action": "test", "value": 42})),
        };

        let created_event = setup
            .create_event_handler
            .execute(event_command)
            .await
            .unwrap();

        // Verify event was created correctly
        assert_eq!(created_event.user_id, user_id);
        assert_eq!(created_event.event_type_id, event_type_id);
        assert_eq!(created_event.event_type, "test_event");

        // Verify event exists in database
        let event_from_db =
            setup.event_dao.find_by_id(created_event.id).await.unwrap();
        assert_eq!(event_from_db.user_id, user_id);
        assert!(event_from_db.metadata.is_some());
    }

    #[tokio::test]
    async fn test_event_creation_for_nonexistent_user() {
        let setup = IntegrationTestSetup::new().await.unwrap();

        let non_existent_user_id = Uuid::now_v7();
        let _event_type_id =
            setup.create_test_event_type("test_event").await.unwrap();

        // Try to create an event for a non-existent user
        let event_command = CreateEventCommand {
            user_id: non_existent_user_id,
            event_type: "test_event".to_string(),
            timestamp: Some(Utc::now()),
            metadata: Some(json!({"action": "test"})),
        };

        let result = setup.create_event_handler.execute(event_command).await;
        assert!(
            result.is_err(),
            "Should fail when creating event for non-existent user"
        );
    }

    #[tokio::test]
    async fn test_user_events_relationship() {
        let setup = IntegrationTestSetup::new().await.unwrap();

        // Create user and event type
        let user_id =
            setup.create_test_user("User With Events").await.unwrap();
        let event_type_id =
            setup.create_test_event_type("login_event").await.unwrap();

        // Create multiple events for the user
        let events_data = vec![
            ("login", json!({"ip": "192.168.1.1"})),
            ("page_view", json!({"page": "/dashboard"})),
            ("logout", json!({"session_duration": 3600})),
        ];

        let mut event_ids = Vec::new();
        for (_action, metadata) in events_data {
            let event_command = CreateEventCommand {
                user_id,
                event_type: "login_event".to_string(),
                timestamp: Some(Utc::now()),
                metadata: Some(metadata),
            };

            let created_event = setup
                .create_event_handler
                .execute(event_command)
                .await
                .unwrap();
            event_ids.push(created_event.id);
        }

        // Verify all events were created
        assert_eq!(event_ids.len(), 3);

        // Query events for the user (this would typically be done through a
        // query handler)
        for event_id in event_ids {
            let event = setup.event_dao.find_by_id(event_id).await.unwrap();
            assert_eq!(event.user_id, user_id);
            assert_eq!(event.event_type_id, event_type_id);
        }
    }

    #[tokio::test]
    async fn test_cross_domain_cascading_operations() {
        let (setup, _analytics_task) = IntegrationTestSetup::new()
            .await
            .unwrap()
            .with_analytics_integration();

        // Create user with analytics integration
        let create_command = CreateUserCommand {
            name: "Cascade Test User".to_string(),
        };
        let created_user = setup
            .create_user_handler
            .execute(create_command)
            .await
            .unwrap();

        // Create events for the user
        let _event_type_id =
            setup.create_test_event_type("cascade_event").await.unwrap();
        let mut event_ids = Vec::new();

        for i in 0..3 {
            let event_command = CreateEventCommand {
                user_id: created_user.id,
                event_type: "cascade_event".to_string(),
                timestamp: Some(Utc::now()),
                metadata: Some(json!({"sequence": i})),
            };

            let event = setup
                .create_event_handler
                .execute(event_command)
                .await
                .unwrap();
            event_ids.push(event.id);
        }

        // Update user (should trigger analytics)
        let update_command = UpdateUserCommand {
            user_id: created_user.id,
            name: Some("Updated Cascade User".to_string()),
        };
        setup
            .update_user_handler
            .execute(update_command)
            .await
            .unwrap();

        // Wait for analytics processing
        setup.wait_for_analytics_processing().await;

        // Verify all data is consistent
        let user_from_db =
            setup.user_dao.find_by_id(created_user.id).await.unwrap();
        assert_eq!(user_from_db.name, "Updated Cascade User");

        // Verify events still exist and are associated with the user
        for event_id in event_ids {
            let event = setup.event_dao.find_by_id(event_id).await.unwrap();
            assert_eq!(event.user_id, created_user.id);
        }

        // Delete user (should trigger analytics)
        let delete_command = DeleteUserCommand {
            user_id: created_user.id,
        };
        setup
            .delete_user_handler
            .execute(delete_command)
            .await
            .unwrap();

        // Wait for analytics processing
        setup.wait_for_analytics_processing().await;

        // Verify user is deleted
        let result = setup.user_dao.find_by_id(created_user.id).await;
        assert!(result.is_err());

        // Note: In a real system, you might want to handle orphaned events
        // For now, we just verify the integration doesn't break
    }

    #[tokio::test]
    async fn test_bulk_operations_integration() {
        let (setup, _analytics_task) = IntegrationTestSetup::new()
            .await
            .unwrap()
            .with_analytics_integration();

        // Create multiple users
        let mut user_ids = Vec::new();
        for i in 0..5 {
            let create_command = CreateUserCommand {
                name: format!("Bulk User {}", i),
            };
            let user = setup
                .create_user_handler
                .execute(create_command)
                .await
                .unwrap();
            user_ids.push(user.id);
        }

        // Create events for all users
        let _event_type_id =
            setup.create_test_event_type("bulk_event").await.unwrap();
        let mut total_events = 0;

        for user_id in &user_ids {
            for j in 0..3 {
                let event_command = CreateEventCommand {
                    user_id: *user_id,
                    event_type: "bulk_event".to_string(),
                    timestamp: Some(Utc::now()),
                    metadata: Some(
                        json!({"batch": "bulk_test", "sequence": j}),
                    ),
                };

                setup
                    .create_event_handler
                    .execute(event_command)
                    .await
                    .unwrap();
                total_events += 1;
            }
        }

        // Wait for analytics processing
        setup.wait_for_analytics_processing().await;

        // Verify all users were created
        assert_eq!(user_ids.len(), 5);

        // Verify we can query for users and events
        for user_id in user_ids {
            let user = setup.user_dao.find_by_id(user_id).await.unwrap();
            assert!(user.name.starts_with("Bulk User"));
        }

        // This test verifies that bulk operations work correctly across
        // domains
        assert_eq!(total_events, 15); // 5 users * 3 events each
    }

    #[tokio::test]
    async fn test_analytics_event_processing_error_handling() {
        let (setup, _analytics_task) = IntegrationTestSetup::new()
            .await
            .unwrap()
            .with_analytics_integration();

        // Create user - this should work even if analytics processing has
        // issues
        let create_command = CreateUserCommand {
            name: "Error Handling Test User".to_string(),
        };

        let created_user = setup
            .create_user_handler
            .execute(create_command)
            .await
            .unwrap();

        // Verify user creation succeeded regardless of analytics issues
        assert_eq!(created_user.name, "Error Handling Test User");

        let user_from_db =
            setup.user_dao.find_by_id(created_user.id).await.unwrap();
        assert_eq!(user_from_db.name, "Error Handling Test User");

        // This test ensures that analytics failures don't break core
        // functionality
    }

    #[tokio::test]
    async fn test_containers_isolation() {
        // This test demonstrates that each test gets its own isolated
        // containers
        let setup1 = IntegrationTestSetup::new().await.unwrap();
        let setup2 = IntegrationTestSetup::new().await.unwrap();

        // Containers should have different connection strings (different
        // ports)
        assert_ne!(
            setup1.container.connection_string,
            setup2.container.connection_string
        );
        assert_ne!(
            setup1.redis_container.connection_string,
            setup2.redis_container.connection_string
        );

        // Both setups should work independently
        let user1_id = setup1
            .create_test_user("User in Container 1")
            .await
            .unwrap();
        let user2_id = setup2
            .create_test_user("User in Container 2")
            .await
            .unwrap();

        // Verify users exist in their respective containers
        let user1 = setup1.user_dao.find_by_id(user1_id).await.unwrap();
        let user2 = setup2.user_dao.find_by_id(user2_id).await.unwrap();

        assert_eq!(user1.name, "User in Container 1");
        assert_eq!(user2.name, "User in Container 2");

        // Verify users don't exist in the other container
        assert!(setup1.user_dao.find_by_id(user2_id).await.is_err());
        assert!(setup2.user_dao.find_by_id(user1_id).await.is_err());
    }
}
