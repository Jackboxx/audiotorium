create sequence audio_playlist_item_position_seq;

alter table audio_playlist_item
    alter column position set default nextval('audio_playlist_item_position_seq');

alter sequence audio_playlist_item_position_seq owned by audio_playlist_item.position;
