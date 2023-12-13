alter table audio_playlist_item
    alter column position drop default;

drop sequence audio_playlist_item_position_seq;
