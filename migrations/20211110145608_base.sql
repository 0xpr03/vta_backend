-- Add migration script here
CREATE TABLE IF NOT EXISTS users
(
    uuid BINARY(16) PRIMARY KEY NOT NULL,
    name VARCHAR(60) COLLATE 'utf8mb4_general_ci' NOT NULL,
    last_seen TIMESTAMP NOT NULL DEFAULT current_timestamp() ON UPDATE current_timestamp(),
    delete_after INT UNSIGNED,
    locked VARCHAR(250),
    INDEX `last_seen` (`last_seen`),
    INDEX `delete_after` (`delete_after`)
);

CREATE TABLE IF NOT EXISTS user_login
(
    user_id BINARY(16) PRIMARY KEY NOT NULL,
    email VARCHAR(319) COLLATE 'utf8mb4_general_ci' NOT NULL UNIQUE,
    password CHAR(82) COLLATE 'ascii_general_ci' NOT NULL,
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
    created BIGINT UNSIGNED NOT NULL DEFAULT current_timestamp(),
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
    created TIMESTAMP NOT NULL DEFAULT current_timestamp(),
    hash BINARY(32),
    UNIQUE INDEX `user_token` (`user_id`,`token_a`),
    INDEX `created` (`created`),
    CONSTRAINT `fk_user_id_pass`
        FOREIGN KEY (user_id) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS user_key
(
    user_id BINARY(16) NOT NULL PRIMARY KEY,
    auth_key BINARY NOT NULL,
    CONSTRAINT `fk_user_id_key`
        FOREIGN KEY (user_id) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS lists
(
    owner BINARY(16) NOT NULL,
    uuid BINARY(16) NOT NULL PRIMARY KEY,
    name VARCHAR(127) NOT NULL,
    changed BIGINT UNSIGNED NOT NULL,
    created BIGINT UNSIGNED NOT NULL,
    INDEX `o_changed` (`owner`,`changed`),
    INDEX `o_created` (`owner`,`created`),
    CONSTRAINT `fk_user_id_list`
        FOREIGN KEY (owner) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS category
(
    owner BINARY(16) NOT NULL,
    uuid BINARY(16) NOT NULL PRIMARY KEY,
    name VARCHAR(127) NOT NULL,
    changed BIGINT UNSIGNED NOT NULL,
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
    changed BIGINT UNSIGNED NOT NULL,
    tip VARCHAR(127),
    INDEX `l_changed` (`list`,`changed`),
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
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS deleted_category
(
    user BINARY(16) NOT NULL,
    category BINARY(16) NOT NULL PRIMARY KEY,
    deleted BIGINT UNSIGNED NOT NULL,
    INDEX `u_deleted` (`user`,`deleted`),
    CONSTRAINT `fk_user_id_delcat`
        FOREIGN KEY (user) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS deleted_list
(
    user BINARY(16) NOT NULL,
    list BINARY(16) NOT NULL PRIMARY KEY,
    deleted BIGINT UNSIGNED NOT NULL,
    INDEX `u_deleted` (`user`,`deleted`),
    CONSTRAINT `fk_user_id_dellist`
        FOREIGN KEY (user) REFERENCES users (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS deleted_entry
(
    list BINARY(16) NOT NULL,
    entry BINARY(16) NOT NULL PRIMARY KEY,
    deleted BIGINT UNSIGNED NOT NULL,
    INDEX `l_deleted` (`list`,`deleted`),
    CONSTRAINT `fk_list_id_delentry`
        FOREIGN KEY (list) REFERENCES lists (uuid)
        ON DELETE CASCADE
        ON UPDATE RESTRICT
);

CREATE TABLE IF NOT EXISTS deleted_user
(
    user BINARY(16) NOT NULL PRIMARY KEY,
    deleted TIMESTAMP NOT NULL DEFAULT current_timestamp()
);