create table audio_metadata (
    identifier varchar(512) primary key,
    name varchar(255),
    author varchar(255),
    duration unsigned integer,
    thumbnail_url varchar(512)
);
