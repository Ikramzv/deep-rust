CONTAINER_NAME="sqlx-postgres-axum"

if [ "$(docker ps -q -f name=$CONTAINER_NAME)" ]; then
    echo "Container is already running"
elif [ "$(docker ps -aq -f status=exited -f name=$CONTAINER_NAME)" ]; then
    # Container exists but is not running, so start it
    echo "Starting existing container..."
    docker start $CONTAINER_NAME
else
    # Container doesn't exist, create and run new one
    echo "Creating new container..."
    docker run \
        -dp 5432:5432 \
        --name $CONTAINER_NAME \
        -e POSTGRES_PASSWORD=postgres \
        postgres
fi
