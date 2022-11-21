//! Run with
//!
//! ```not_rust
//! cd examples && cargo run -p example-simple-with-role
//! ```

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use axum::{
    extract::{FromRequest, RequestParts},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Extension, Router,
};
use axum_login::{
    axum_sessions::{async_session::MemoryStore as SessionMemoryStore, SessionLayer},
    memory_store::MemoryStore as AuthMemoryStore,
    AuthLayer, AuthUser, RequireAuthorizationLayer,
};
use rand::Rng;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum Role {
    User,
    Admin,
}

#[derive(Debug, Clone)]
struct User {
    id: usize,
    password_hash: String,
    role: Role,
    name: String,
}

impl User {
    fn get_rusty_user() -> Self {
        Self {
            id: 1,
            name: "Ferris the Crab".to_string(),
            password_hash: "password".to_string(),
            role: Role::Admin,
        }
    }
}

impl AuthUser<Role> for User {
    fn get_id(&self) -> String {
        format!("{}", self.id)
    }

    fn get_password_hash(&self) -> String {
        self.password_hash.clone()
    }

    fn get_role(&self) -> Option<Role> {
        Some(self.role.clone())
    }
}

/// Example how to create an Admin user guard
/// Can be modified to support any type of permission
struct RequireAdmin(User);
struct RequireUser(User);

#[async_trait]
impl<B> FromRequest<B> for RequireAdmin
where
    B: Send + 'static,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(user): Extension<User> = Extension::from_request(req)
            .await
            .map_err(|_err| StatusCode::FORBIDDEN)?;

        if user
            .get_role()
            .map_or(false, |role| matches!(role, Role::Admin))
        {
            Ok(RequireAdmin(user))
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for RequireUser
where
    B: Send + 'static,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(user): Extension<User> = Extension::from_request(req)
            .await
            .map_err(|_err| StatusCode::FORBIDDEN)?;

        if user
            .get_role()
            .map_or(false, |role| matches!(role, Role::User))
        {
            Ok(RequireUser(user))
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}

type AuthContext = axum_login::extractors::AuthContext<User, AuthMemoryStore<User>, Role>;

#[tokio::main]
async fn main() {
    let secret = rand::thread_rng().gen::<[u8; 64]>();

    let session_store = SessionMemoryStore::new();
    let session_layer = SessionLayer::new(session_store, &secret).with_secure(false);

    let store = Arc::new(RwLock::new(HashMap::default()));
    let user = User::get_rusty_user();

    store.write().await.insert(user.get_id(), user);

    let user_store = AuthMemoryStore::new(&store);
    let auth_layer = AuthLayer::new(user_store, &secret);

    async fn login_handler(mut auth: AuthContext) {
        auth.login(&User::get_rusty_user()).await.unwrap();
    }

    async fn logout_handler(mut auth: AuthContext) {
        dbg!("Logging out user: {}", &auth.current_user);
        auth.logout().await;
    }

    async fn protected_handler(Extension(user): Extension<User>) -> impl IntoResponse {
        format!("Logged in as: {}", user.name)
    }

    async fn admin_handler(RequireAdmin(user): RequireAdmin) -> impl IntoResponse {
        format!("Admin logged in as: {}", user.name)
    }

    async fn user_handler(RequireUser(user): RequireUser) -> impl IntoResponse {
        format!("User logged in as: {}", user.name)
    }

    let app = Router::new()
        .route("/protected", get(protected_handler))
        .route("/protected_admin", get(admin_handler))
        .route("/protected_user", get(user_handler))
        .route_layer(RequireAuthorizationLayer::<User, Role>::login())
        .route("/login", get(login_handler))
        .route("/logout", get(logout_handler))
        .layer(auth_layer)
        .layer(session_layer);

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
