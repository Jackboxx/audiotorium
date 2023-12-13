alter table audio_playlist_item
    add constraint unq_audio_playlist_item_position_in_playlist unique (playlist_identifier, position);
