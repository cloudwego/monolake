# stop traefik proxy service
kill -15 $(ps aux | grep 'traefik' | awk '{print $2}')
