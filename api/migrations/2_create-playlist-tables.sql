create table if not exists audio_playlist (
    identifier varchar(512) primary key,
    name varchar(255),
    author varchar(255),
    cover_art_url varchar(512)
);

create table if not exists audio_playlist_item (
    playlist_identifier varchar(512),
    item_identifier varchar(512),
    constraint fk_audio_playlist
        foreign key(playlist_identifier) 
	    references audio_playlist(identifier)
        on delete cascade,
    constraint fk_audio_metadata 
        foreign key(item_identifier) 
	    references audio_metadata(identifier)
        on delete cascade,
	primary key (playlist_identifier, item_identifier)
);
