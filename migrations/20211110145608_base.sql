-- Add migration script here
CREATE TABLE IF NOT EXISTS users
(
    uuid BINARY(16) PRIMARY KEY NOT NULL,
    name VARCHAR(60) COLLATE 'utf8mb4_general_ci' NOT NULL,
    last_seen DATETIME NOT NULL DEFAULT current_timestamp() ON UPDATE current_timestamp(),
    delete_after INT UNSIGNED,
    locked VARCHAR(250),
    INDEX `last_seen` (`last_seen`),
    INDEX `delete_after` (`delete_after`)
);

CREATE TABLE IF NOT EXISTS user_login
(
    user_id BINARY(16) PRIMARY KEY NOT NULL,
    email VARCHAR(319) COLLATE 'utf8mb4_general_ci' NOT NULL UNIQUE,
    password VARCHAR(255) COLLATE 'ascii_general_ci' NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT `fk_user_id`
        FOREIGN KEY (user_id) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS verify_token
(
    user_id BINARY(16) NOT NULL,
    token VARCHAR(255) NOT NULL,
    created DATETIME NOT NULL DEFAULT current_timestamp(),
    INDEX `user_token` (`user_id`,`token`),
    INDEX `created` (`created`),
    CONSTRAINT `fk_user_id_verify`
        FOREIGN KEY (user_id) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS password_reset
(
    user_id BINARY(16) NOT NULL,
    token_a VARCHAR(30) NOT NULL UNIQUE,
    created DATETIME NOT NULL DEFAULT current_timestamp(),
    `hash` BINARY(32),
    UNIQUE INDEX `user_token` (`user_id`,`token_a`),
    INDEX `created` (`created`),
    CONSTRAINT `fk_user_id_pass`
        FOREIGN KEY (user_id) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS key_type
(
    id INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(255) NOT NULL
);

CREATE TABLE IF NOT EXISTS user_key
(
    user_id BINARY(16) NOT NULL PRIMARY KEY,
    auth_key VARBINARY(400) NOT NULL,
    key_type INT NOT NULL,
    CONSTRAINT `fk_user_id_key`
        FOREIGN KEY (user_id) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT,
    CONSTRAINT `fk_key_type_key`
        FOREIGN KEY (key_type) REFERENCES key_type (id)
        ON DELETE RESTRICT
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS last_synced
(
    user_id BINARY(16) NOT NULL,
    client BINARY(16) NOT NULL,
    date DATETIME NOT NULL DEFAULT current_timestamp(),
    `type` INT NOT NULL,
    PRIMARY KEY (user_id,`type`,client),
    CONSTRAINT `fk_user_id_last_synced`
        FOREIGN KEY (user_id) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS lists
(
    owner BINARY(16) NOT NULL,
    uuid BINARY(16) NOT NULL PRIMARY KEY,
    name VARCHAR(127) NOT NULL,
    name_a VARCHAR(127) NOT NULL,
    name_b VARCHAR(127) NOT NULL,
    changed DATETIME NOT NULL,
    created DATETIME NOT NULL,
    INDEX `o_changed` (`owner`,`changed`),
    INDEX `o_created` (`owner`,`created`),
    INDEX (`uuid`,`owner`),
    INDEX (uuid,changed),
    INDEX (`owner`, changed, uuid),
    CONSTRAINT `fk_user_id_list`
        FOREIGN KEY (owner) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS list_permissions
(
    user BINARY(16) NOT NULL,
    list BINARY(16) NOT NULL,
    `write` BOOLEAN NOT NULL,
    reshare BOOLEAN NOT NULL,
    changed DATETIME NOT NULL,
    PRIMARY KEY (list,user),
    INDEX (list),
    INDEX (user,list,`write`),
    CONSTRAINT `fk_user_id_list_permissions`
        FOREIGN KEY (user) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT,
    CONSTRAINT `fk_list_list_permissions`
        FOREIGN KEY (list) REFERENCES lists (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS share_token
(
    list BINARY(16) NOT NULL,
    token_a BINARY(16) NOT NULL PRIMARY KEY,
    deadline DATETIME NOT NULL,
    `hash` BINARY(32),
    `write` BOOLEAN NOT NULL,
    reshare BOOLEAN NOT NULL,
    reusable BOOLEAN NOT NULL,
    INDEX `outdateds` (`deadline`),
    CONSTRAINT `fk_share_token_list`
        FOREIGN KEY (list) REFERENCES lists (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS category
(
    owner BINARY(16) NOT NULL,
    uuid BINARY(16) NOT NULL PRIMARY KEY,
    name VARCHAR(127) NOT NULL,
    changed DATETIME NOT NULL,
    INDEX `o_changed` (`owner`,`changed`),
    CONSTRAINT `fk_user_id_category`
        FOREIGN KEY (owner) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS list_category
(
    list BINARY(16) NOT NULL,
    category BINARY(16) NOT NULL,
    PRIMARY KEY (list,category),
    CONSTRAINT `fk_list_id_lc`
        FOREIGN KEY (list) REFERENCES lists (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT,
    CONSTRAINT `fk_category_id_lc`
        FOREIGN KEY (category) REFERENCES category (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS entries
(
    list BINARY(16) NOT NULL,
    uuid BINARY(16) NOT NULL PRIMARY KEY,
    changed DATETIME NOT NULL,
    updated DATETIME NOT NULL,
    tip VARCHAR(127),
    INDEX `l_updated` (`list`,`updated`),
    INDEX (`uuid`,`updated`),
    INDEX (list),
    CONSTRAINT `fk_list_id_entry`
        FOREIGN KEY (list) REFERENCES lists (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS entry_meaning
(
    entry BINARY(16) NOT NULL,
    value VARCHAR(120) NOT NULL,
    is_a BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT `fk_user_id_meaning`
        FOREIGN KEY (entry) REFERENCES entries (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT,
    INDEX(entry)
);

CREATE TABLE IF NOT EXISTS deleted_category
(
    user BINARY(16) NOT NULL,
    category BINARY(16) NOT NULL PRIMARY KEY,
    created DATETIME NOT NULL,
    INDEX `u_deleted` (`user`,`created`),
    CONSTRAINT `fk_user_id_delcat`
        FOREIGN KEY (user) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS deleted_list
(
    user BINARY(16) NOT NULL,
    list BINARY(16) NOT NULL PRIMARY KEY,
    created DATETIME NOT NULL,
    INDEX `u_deleted` (`user`,`created`),
    INDEX (`user`,`list`),
    INDEX (list),
    CONSTRAINT `fk_user_id_dellist`
        FOREIGN KEY (user) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS deleted_list_shared
(
    user BINARY(16) NOT NULL,
    list BINARY(16) NOT NULL,
    created DATETIME NOT NULL,
    PRIMARY KEY (user,list),
    INDEX (list),
    INDEX (user),
    CONSTRAINT `fk_user_id_dellist_shared`
        FOREIGN KEY (user) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS deleted_entry
(
    list BINARY(16) NOT NULL,
    `entry` BINARY(16) NOT NULL PRIMARY KEY,
    created DATETIME NOT NULL,
    INDEX `l_deleted` (`list`,`created`),
    INDEX (list),
    CONSTRAINT `fk_list_id_delentry`
        FOREIGN KEY (list) REFERENCES lists (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS deleted_user
(
    user BINARY(16) NOT NULL PRIMARY KEY,
    created DATETIME NOT NULL DEFAULT current_timestamp()
);

CREATE TABLE IF NOT EXISTS settings
(
    `key` VARCHAR(127) NOT NULL PRIMARY KEY,
    `value` VARCHAR(127) NOT NULL
);