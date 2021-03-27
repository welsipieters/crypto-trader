CREATE TABLE transactions (
                              id char(36) NOT NULL PRIMARY KEY,
                              exchange_name varchar(64) NOT NULL,
                              buy_exchange_id varchar(64),
                              sell_exchange_id varchar(64),
                              amount double(8,2) NOT NULL,
                              symbol varchar(16) NOT NULL,
                              price double(8,2) NOT NULL,
                              stage varchar(255) NOT NULL,
                              created_at timestamp NULL DEFAULT NULL,
                              updated_at timestamp NULL DEFAULT NULL
)