
# Projet Chat Temps Réel en Rust
# 💬 Projet de Chat en Temps Réel en Rust

Ce projet est une application web de chat en temps réel développée entièrement en **Rust**, avec une interface HTML/CSS/JavaScript. Il utilise **Actix Web** pour le serveur HTTP/WebSocket et **SQLx (SQLite)** pour la base de données.

## 🛠️ Fonctionnalités principales

- Authentification sécurisée (hashage des mots de passe avec Argon2)
- Chat en temps réel avec WebSocket
- Gestion des sessions via cookies sécurisés
- Base de données SQLite initialisée par script
- Interface web responsive en HTML/CSS/JavaScript
- Templates dynamiques avec Tera
- Architecture modulaire et sécurisée (CSRF, XSS, injections SQL évitées)

---

## 🚀 Lancer le projet

### 1. Cloner le dépôt

```bash
git clone https://github.com/GLuck777/Comunik.git
cd Comunik
```
2. 🗃️ Initialiser la base de données SQLite

```bash
sqlite3 app.db < db/init.sql
```

3. 🔧 Lancer le serveur

```bash
cargo build
cargo run
```

Le serveur sera disponible sur: [http://localhost:8080](http://localhost:8080)

## 🧪 Tester WebSocket

Utilise le formulaire sur la page d'accueil pour envoyer des messages qui seront renvoyés par le serveur en temps réel.


<!-- actix-web : pour les serveurs web. -->
<!-- actix-session : pour gérer les sessions utilisateur. -->
<!-- tera : pour le moteur de templates. -->
<!-- uuid : pour générer un UUID unique pour chaque utilisateur. -->

## 🔄 WebSocket : Tester la messagerie en temps réel

    Connectez-vous à votre profil
    
    Rendez-vous sur la page d’accueil.

    Choississez une room

    Entrez un message dans le formulaire de la room.

    Le message est transmis en temps réel à tous les clients connectés via WebSocket.

## 🧱 Technologies et dépendances clés

    actix-web : framework HTTP performant

    actix-web-actors : gestion des WebSockets

    sqlx avec SQLite : ORM asynchrone

    tera : moteur de templates HTML

    argon2 : sécurité des mots de passe

    actix-session : sessions utilisateurs sécurisées

    [uuid, chrono, serde, tokio] pour divers utilitaires

🛡️ Sécurité

    Mot de passe haché avec Argon2

    Jeton CSRF pour tous les formulaires

    Nettoyage des données utilisateur (anti-XSS et SQL injection)

    Sessions stockées dans des cookies chiffrés

## 📚 Documentation

    doc/architecture.md : architecture logicielle

    doc/routes.md : documentation des endpoints HTTP/WebSocket

    [doc/MCD_graph.jpg, MLD_graph.jpg, MPD_graph.jpg] : modèle de données [Modèle Conceptuel de Données, Modèle logique de Données, Modèle Physique de Données]

    Document URL pratique pour débuter: 
    {
        [build websocket server with rust](https://blog.logrocket.com/build-websocket-server-with-rust/),
        [Gain to do websocket with rust](https://blog.logrocket.com/build-websocket-server-with-rust/)
    }

## 🧑‍💻 Auteur

Développé par [Ton Nom / pseudo] – 100% solo dev 👨‍💻 
[GLuck777](https://github.com/GLuck777)