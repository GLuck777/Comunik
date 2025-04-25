
# Projet Chat Temps Réel en Rust

## 🔧 Démarrage du projet

```bash
cargo build
cargo run
```

Accéder au client WebSocket : [http://localhost:8080](http://localhost:8080)

## 🗃️ Initialiser la base de données

Créer la base SQLite avec le script `db/init.sql` :

```bash
sqlite3 app.db < db/init.sql
```

## 🧪 Tester WebSocket

Utilise le formulaire sur la page d'accueil pour envoyer des messages qui seront renvoyés par le serveur en temps réel.


<!-- actix-web : pour les serveurs web. -->
<!-- actix-session : pour gérer les sessions utilisateur. -->
<!-- tera : pour le moteur de templates. -->
<!-- uuid : pour générer un UUID unique pour chaque utilisateur. -->