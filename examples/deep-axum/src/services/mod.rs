// tower::Service

// is roughly equivalent to an async function that takes a Request and returns a Response

// async fn a(Request) -> Result<Response, E> for some <Request, E>
// async fn b(Request) -> Result<Response, E> for some <Request, E>

// async fn c(req: Request) -> Result<Response, E> {
//     (a.layer(b))(req).await
// }
