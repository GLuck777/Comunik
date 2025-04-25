async fn generate_unique_pseudo(pool: &SqlitePool) -> String {
    loop {
        let pseudo: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(15)
            .map(char::from)
            .collect();

        // Vérifie si le pseudo existe déjà
        let exists: Option<(String,)> = sqlx::query_as("SELECT pseudo FROM users WHERE pseudo = ?")
            .bind(&pseudo)
            .fetch_optional(pool)
            .await
            .unwrap();

        if exists.is_none() {
            return pseudo;
        }
    }
}