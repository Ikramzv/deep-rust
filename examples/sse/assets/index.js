function main() {
  const messageContainer = document.querySelector(".message-container");
  const container = document.querySelector(".container");

  function addRow(message, type = "data") {
    const messageElement = document.createElement("div");
    messageElement.classList.add("message");
    messageElement.classList.add(type);
    messageElement.textContent = message;
    messageContainer.appendChild(messageElement);

    window.scrollTo({
      top: container.scrollHeight,
    });
  }

  const eventSource = new EventSource("http://localhost:3000/sse");

  eventSource.addEventListener("message", (event) => {
    console.log("message", event);
    addRow(event.data, "data");
  });

  eventSource.addEventListener("error", (event) => {
    console.log("error", event);
    addRow("Error: something went wrong", "error");
  });

  eventSource.addEventListener("open", (event) => {
    console.log("open", event);
    addRow("Connected to the server", "data");
  });
}

main();
