source "app"
{
    type = "SQL";
    database = "./app.db";
    user = "admin";
    password = "";
}

table "users"
{
    primary_key = "uuid";
    field "uuid" { type = "TEXT"; unique = true; }
    field "email" { type = "TEXT"; unique = true; }
    field "password" { type = "TEXT"; }
    field "pseudo" { type = "TEXT"; unique = true; }
    field "created_at" { type = "DATETIME"; default = "CURRENT_TIMESTAMP"; }
}

table "rooms"
{
    primary_key = "room_uuid";
    field "room_uuid" { type = "TEXT"; unique = true; }
    field "name" { type = "TEXT"; }
    field "owner_uuid" { type = "TEXT"; }
    field "created_at" { type = "DATETIME"; default = "CURRENT_TIMESTAMP"; }
    foreign_key "owner_uuid" references "users" ("uuid");
}

table "messages"
{
    primary_key = "id";
    field "id" { type = "INTEGER"; auto_increment = true; }
    field "room_uuid" { type = "TEXT"; }
    field "user_uuid" { type = "TEXT"; }
    field "content" { type = "TEXT"; }
    field "created_at" { type = "DATETIME"; default = "CURRENT_TIMESTAMP"; }
    foreign_key "room_uuid" references "rooms" ("room_uuid") on_delete = "CASCADE";
    foreign_key "user_uuid" references "users" ("uuid");
}

table "room_members"
{
    primary_key = "room_uuid, user_uuid";
    field "room_uuid" { type = "TEXT"; }
    field "user_uuid" { type = "TEXT"; }
    field "role" { type = "TEXT"; default = "member"; }
    foreign_key "room_uuid" references "rooms" ("room_uuid") on_delete = "CASCADE";
    foreign_key "user_uuid" references "users" ("uuid");
}

table "friend_requests"
{
    primary_key = "sender_uuid, receiver_uuid";
    field "sender_uuid" { type = "TEXT"; }
    field "receiver_uuid" { type = "TEXT"; }
    field "status" { type = "TEXT"; default = "pending"; }
    field "created_at" { type = "DATETIME"; default = "CURRENT_TIMESTAMP"; }
    foreign_key "sender_uuid" references "users" ("uuid");
    foreign_key "receiver_uuid" references "users" ("uuid");
}

table "friendships"
{
    primary_key = "user1_uuid, user2_uuid";
    field "user1_uuid" { type = "TEXT"; }
    field "user2_uuid" { type = "TEXT"; }
    field "created_at" { type = "DATETIME"; default = "CURRENT_TIMESTAMP"; }
    foreign_key "user1_uuid" references "users" ("uuid");
    foreign_key "user2_uuid" references "users" ("uuid");
}

table "notifications"
{
    primary_key = "id";
    field "id" { type = "INTEGER"; auto_increment = true; }
    field "user_uuid" { type = "TEXT"; }
    field "message" { type = "TEXT"; }
    field "created_at" { type = "DATETIME"; default = "CURRENT_TIMESTAMP"; }
    foreign_key "user_uuid" references "users" ("uuid");
}
