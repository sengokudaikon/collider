use chrono::Utc;
use database_traits::dao::GenericDao;
use events_commands::CreateEventCommand;
use flume::Sender;
use sql_connection::SqlConnect;
use tracing::{instrument, warn};
use user_commands::{
    CreateUserCommand, DeleteUserCommand, UpdateUserCommand,
};
use user_dao::UserDao;
use user_errors::UserError;
use user_events::UserAnalyticsEvent;
use user_responses::UserResponse;

#[derive(Clone)]
pub struct CreateUserHandler {
    user_dao: UserDao,
    analytics_event_sender: Option<Sender<UserAnalyticsEvent>>,
    event_sender: Option<Sender<CreateEventCommand>>,
}

impl CreateUserHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
            analytics_event_sender: None,
            event_sender: None,
        }
    }

    pub fn with_analytics_event_sender(
        mut self, analytics_event_sender: Sender<UserAnalyticsEvent>,
    ) -> Self {
        self.analytics_event_sender = Some(analytics_event_sender);
        self
    }

    pub fn with_event_sender(
        mut self, event_sender: Sender<CreateEventCommand>,
    ) -> Self {
        self.event_sender = Some(event_sender);
        self
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: CreateUserCommand,
    ) -> Result<UserResponse, UserError> {
        let saved_user = self.user_dao.create(command).await?;

        if let Some(analytics_event_sender) = &self.analytics_event_sender {
            let analytics_event = UserAnalyticsEvent::UserCreated {
                user_id: saved_user.id,
                name: saved_user.name.clone(),
                created_at: saved_user.created_at,
                registration_source: None, /* TODO: extract from request
                                            * context if needed */
            };
            if let Err(e) = analytics_event_sender.send(analytics_event) {
                warn!("Failed to emit user analytics event: {}", e);
            }
        }

        Ok(UserResponse {
            id: saved_user.id,
            name: saved_user.name,
            created_at: saved_user.created_at,
        })
    }
}

#[derive(Clone)]
pub struct UpdateUserHandler {
    user_dao: UserDao,
    analytics_event_sender: Option<Sender<UserAnalyticsEvent>>,
    event_sender: Option<Sender<CreateEventCommand>>,
}

impl UpdateUserHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
            analytics_event_sender: None,
            event_sender: None,
        }
    }

    pub fn with_analytics_event_sender(
        mut self, analytics_event_sender: Sender<UserAnalyticsEvent>,
    ) -> Self {
        self.analytics_event_sender = Some(analytics_event_sender);
        self
    }

    pub fn with_event_sender(
        mut self, event_sender: Sender<CreateEventCommand>,
    ) -> Self {
        self.event_sender = Some(event_sender);
        self
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: UpdateUserCommand,
    ) -> Result<UserResponse, UserError> {
        let existing_user =
            self.user_dao.find_by_id(command.user_id).await.map_err(
                |_| {
                    UserError::NotFound {
                        user_id: command.user_id,
                    }
                },
            )?;
        let name_updated = command.name.is_some();
        let user_id = command.user_id;
        let old_name = existing_user.name.clone();

        let updated_user =
            self.user_dao.update(command.user_id, command).await?;

        // Emit analytics event if name was updated
        if name_updated {
            if let Some(analytics_event_sender) = &self.analytics_event_sender
            {
                let analytics_event = UserAnalyticsEvent::UserNameUpdated {
                    user_id,
                    old_name,
                    new_name: updated_user.name.clone(),
                    updated_at: Utc::now(),
                };
                if let Err(e) = analytics_event_sender.send(analytics_event) {
                    warn!("Failed to emit user analytics event: {}", e);
                }
            }
        }

        Ok(UserResponse {
            id: updated_user.id,
            name: updated_user.name,
            created_at: updated_user.created_at,
        })
    }
}

#[derive(Clone)]
pub struct DeleteUserHandler {
    user_dao: UserDao,
    analytics_event_sender: Option<Sender<UserAnalyticsEvent>>,
    event_sender: Option<Sender<CreateEventCommand>>,
}

impl DeleteUserHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            user_dao: UserDao::new(db),
            analytics_event_sender: None,
            event_sender: None,
        }
    }

    pub fn with_analytics_event_sender(
        mut self, analytics_event_sender: Sender<UserAnalyticsEvent>,
    ) -> Self {
        self.analytics_event_sender = Some(analytics_event_sender);
        self
    }

    pub fn with_event_sender(
        mut self, event_sender: Sender<CreateEventCommand>,
    ) -> Self {
        self.event_sender = Some(event_sender);
        self
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: DeleteUserCommand,
    ) -> Result<(), UserError> {
        let _existing_user =
            self.user_dao.find_by_id(command.user_id).await.map_err(
                |_| {
                    UserError::NotFound {
                        user_id: command.user_id,
                    }
                },
            )?;

        self.user_dao.delete(command.user_id).await?;

        // Emit analytics event for user deletion
        if let Some(analytics_event_sender) = &self.analytics_event_sender {
            let analytics_event = UserAnalyticsEvent::UserDeleted {
                user_id: command.user_id,
                deleted_at: Utc::now(),
            };
            if let Err(e) = analytics_event_sender.send(analytics_event) {
                warn!("Failed to emit user analytics event: {}", e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use flume;
    use test_utils::*;
    use user_commands::{DeleteUserCommand, UpdateUserCommand};
    use user_events::UserAnalyticsEvent;

    use super::*;

    async fn setup_test_handlers() -> anyhow::Result<(
        test_utils::TestPostgresContainer,
        CreateUserHandler,
        UpdateUserHandler,
        DeleteUserHandler,
    )> {
        let container = test_utils::TestPostgresContainer::new().await?;
        let sql_connect = create_sql_connect(&container);

        let create_handler = CreateUserHandler::new(sql_connect.clone());
        let update_handler = UpdateUserHandler::new(sql_connect.clone());
        let delete_handler = DeleteUserHandler::new(sql_connect);

        Ok((container, create_handler, update_handler, delete_handler))
    }

    #[tokio::test]
    async fn test_create_user_handler() {
        let (_container, create_handler, ..) =
            setup_test_handlers().await.unwrap();

        let command = CreateUserCommand {
            name: "test_user".to_string(),
        };

        let result = create_handler.execute(command).await.unwrap();

        assert_eq!(result.name, "test_user");
        assert!(!result.id.is_nil());
    }

    #[tokio::test]
    async fn test_user_analytics_event_publishing() {
        let (_container, create_handler, ..) =
            setup_test_handlers().await.unwrap();

        // Set up analytics event channel
        let (analytics_event_sender, analytics_event_receiver) =
            flume::unbounded();
        let create_handler = create_handler
            .with_analytics_event_sender(analytics_event_sender);

        let command = CreateUserCommand {
            name: "analytics_event_test_user".to_string(),
        };

        let result = create_handler.execute(command).await.unwrap();

        // Verify user was created
        assert_eq!(result.name, "analytics_event_test_user");

        // Verify analytics event was published
        let published_event = analytics_event_receiver.try_recv().unwrap();
        match published_event {
            UserAnalyticsEvent::UserCreated { user_id, name, .. } => {
                assert_eq!(user_id, result.id);
                assert_eq!(name, "analytics_event_test_user");
            }
            _ => panic!("Expected UserCreated analytics event"),
        }
    }

    #[tokio::test]
    async fn test_update_user_analytics_event() {
        let (_container, create_handler, update_handler, _) =
            setup_test_handlers().await.unwrap();

        // Create a user first
        let create_command = CreateUserCommand {
            name: "update_analytics_test_user".to_string(),
        };
        let created_user =
            create_handler.execute(create_command).await.unwrap();

        // Set up analytics event channel for updates
        let (analytics_event_sender, analytics_event_receiver) =
            flume::unbounded();
        let update_handler = update_handler
            .with_analytics_event_sender(analytics_event_sender);

        // Update the user
        let update_command = UpdateUserCommand {
            user_id: created_user.id,
            name: Some("updated_analytics_test_user".to_string()),
        };
        let result = update_handler.execute(update_command).await.unwrap();

        // Verify user was updated
        assert_eq!(result.name, "updated_analytics_test_user");

        // Verify analytics event was published
        let published_event = analytics_event_receiver.try_recv().unwrap();
        match published_event {
            UserAnalyticsEvent::UserNameUpdated {
                user_id,
                old_name,
                new_name,
                ..
            } => {
                assert_eq!(user_id, created_user.id);
                assert_eq!(old_name, "update_analytics_test_user");
                assert_eq!(new_name, "updated_analytics_test_user");
            }
            _ => panic!("Expected UserNameUpdated analytics event"),
        }
    }

    #[tokio::test]
    async fn test_delete_user_analytics_event() {
        let (_container, create_handler, _, delete_handler) =
            setup_test_handlers().await.unwrap();

        // Create a user first
        let create_command = CreateUserCommand {
            name: "delete_analytics_test_user".to_string(),
        };
        let created_user =
            create_handler.execute(create_command).await.unwrap();

        // Set up analytics event channel for deletes
        let (analytics_event_sender, analytics_event_receiver) =
            flume::unbounded();
        let delete_handler = delete_handler
            .with_analytics_event_sender(analytics_event_sender);

        // Delete the user
        let delete_command = DeleteUserCommand {
            user_id: created_user.id,
        };
        let result = delete_handler.execute(delete_command).await;

        // Verify deletion was successful
        assert!(result.is_ok());

        // Verify analytics event was published
        let published_event = analytics_event_receiver.try_recv().unwrap();
        match published_event {
            UserAnalyticsEvent::UserDeleted { user_id, .. } => {
                assert_eq!(user_id, created_user.id);
            }
            _ => panic!("Expected UserDeleted analytics event"),
        }
    }
}
