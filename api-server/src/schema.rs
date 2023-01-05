// @generated automatically by Diesel CLI.

diesel::table! {
    files (file_name) {
        file_name -> Text,
        file_type -> Nullable<Text>,
        file_upload_date -> Integer,
    }
}
