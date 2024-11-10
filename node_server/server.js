const http = require('http');

const server = http.createServer((req, res) => {
    // Log all request headers
    console.log('Request Headers:', req.headers);

    // Set the response header and send the "Hello, World!" message
    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end('Hello, World! (node)');
});

const PORT = 3000;
server.listen(PORT, () => {
    console.log(`Server is running on http://localhost:${PORT}`);
});
