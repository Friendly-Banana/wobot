# allow manpage installation
sed -i '/path-exclude \/usr\/share\/man/d' /etc/dpkg/dpkg.cfg.d/docker
sed -i '/path-exclude \/usr\/share\/groff/d' /etc/dpkg/dpkg.cfg.d/docker

# add non-free
sed -i 's/Components: main/Components: main non-free/' /etc/apt/sources.list.d/debian.sources
apt update
apt install -y libopus-dev man manpages-dev manpages-posix manpages-posix-dev
apt install --reinstall coreutils

curl -L https://yt-dl.org/downloads/latest/youtube-dl -o /usr/local/bin/youtube-dl
chmod a+rx /usr/local/bin/youtube-dl