FROM archlinux:latest

WORKDIR /root

RUN pacman -Sy --noconfirm wireplumber pipewire pipewire-alsa pipewire-audio vlc 

COPY audio.opus /
RUN bash
