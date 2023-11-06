create table if not exists audio_metadata (
    identifier varchar(512) primary key,
    name varchar(255),
    author varchar(255),
    duration integer,
    cover_art_url varchar(512)
);
