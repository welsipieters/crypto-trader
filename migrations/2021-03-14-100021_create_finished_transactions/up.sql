-- Your SQL goes here
CREATE TABLE finished_transactions (
     id char(36) COLLATE utf8mb4_unicode_ci NOT NULL,
     transaction_id char(36) COLLATE utf8mb4_unicode_ci NOT NULL,
     amount_bought double(8,2) NOT NULL,
     buy_price double(8,2) NOT NULL,
     amount_sold double(8,2) NOT NULL,
     sell_price double(8,2) NOT NULL,
     created_at timestamp NULL DEFAULT NULL,
     updated_at timestamp NULL DEFAULT NULL,
     PRIMARY KEY (id),
     KEY finished_transactions_transaction_id_foreign (transaction_id),
     CONSTRAINT finished_transactions_transaction_id_foreign FOREIGN KEY (transaction_id) REFERENCES transactions (sell_exchange_id)
);