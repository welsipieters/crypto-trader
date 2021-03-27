table! {
    finished_transactions (id) {
        id -> Char,
        transaction_id -> Char,
        amount_bought -> Double,
        buy_price -> Double,
        amount_sold -> Double,
        sell_price -> Double,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}

table! {
    transactions (id) {
        id -> Char,
        exchange_name -> Varchar,
        buy_exchange_id -> Nullable<Varchar>,
        sell_exchange_id -> Nullable<Varchar>,
        amount -> Double,
        symbol -> Varchar,
        price -> Double,
        stage -> Varchar,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}

joinable!(finished_transactions -> transactions (transaction_id));

allow_tables_to_appear_in_same_query!(finished_transactions, transactions,);
