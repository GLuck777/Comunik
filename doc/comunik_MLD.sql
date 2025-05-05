CREATE TABLE `users` (
  `uuid` varchar(255) UNIQUE PRIMARY KEY NOT NULL,
  `email` varchar(255) UNIQUE NOT NULL,
  `password` varchar(255) NOT NULL,
  `pseudo` varchar(255) UNIQUE NOT NULL,
  `created_at` timestamp DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE `rooms` (
  `room_uuid` varchar(255) UNIQUE PRIMARY KEY NOT NULL,
  `name` varchar(255) NOT NULL,
  `owner_uuid` varchar(255) NOT NULL,
  `created_at` timestamp DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE `messages` (
  `room_uuid` varchar(255) NOT NULL,
  `user_uuid` varchar(255) NOT NULL,
  `content` text NOT NULL,
  `created_at` timestamp DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE `room_members` (
  `room_uuid` varchar(255) NOT NULL,
  `user_uuid` varchar(255) NOT NULL,
  `role` varchar(255) DEFAULT 'member',
  `created_at` timestamp DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE `friend_requests` (
  `sender_uuid` varchar(255) NOT NULL,
  `receiver_uuid` varchar(255) NOT NULL,
  `status` varchar(255) DEFAULT 'pending',
  `created_at` timestamp DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE `friendships` (
  `user1_uuid` varchar(255) NOT NULL,
  `user2_uuid` varchar(255) NOT NULL,
  `created_at` timestamp DEFAULT (CURRENT_TIMESTAMP)
);

CREATE TABLE `notifications` (
  `id` integer PRIMARY KEY,
  `user_uuid` varchar(255) NOT NULL,
  `message` text NOT NULL,
  `type` varchar(255) DEFAULT 'pending',
  `created_at` timestamp DEFAULT (CURRENT_TIMESTAMP)
);

ALTER TABLE `room_members` COMMENT = 'Composite primary key on (room_id, user_uuid)';

ALTER TABLE `rooms` ADD FOREIGN KEY (`owner_uuid`) REFERENCES `users` (`uuid`);

ALTER TABLE `messages` ADD FOREIGN KEY (`room_uuid`) REFERENCES `rooms` (`room_uuid`);

ALTER TABLE `messages` ADD FOREIGN KEY (`user_uuid`) REFERENCES `users` (`uuid`);

ALTER TABLE `room_members` ADD FOREIGN KEY (`room_uuid`) REFERENCES `rooms` (`room_uuid`);

ALTER TABLE `room_members` ADD FOREIGN KEY (`user_uuid`) REFERENCES `users` (`uuid`);

ALTER TABLE `friend_requests` ADD FOREIGN KEY (`sender_uuid`) REFERENCES `users` (`uuid`);

ALTER TABLE `friend_requests` ADD FOREIGN KEY (`receiver_uuid`) REFERENCES `users` (`uuid`);

ALTER TABLE `friendships` ADD FOREIGN KEY (`user1_uuid`) REFERENCES `users` (`uuid`);

ALTER TABLE `friendships` ADD FOREIGN KEY (`user2_uuid`) REFERENCES `users` (`uuid`);

ALTER TABLE `notifications` ADD FOREIGN KEY (`user_uuid`) REFERENCES `users` (`uuid`);