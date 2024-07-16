# stop monolake proxy service
kill -15 $(ps aux | grep 'monolake' | awk '{print $2}')
