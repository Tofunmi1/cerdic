// The only wasi imports the C++ side needs after linking; none are used on the happy path.
export const fd_close = () => 8, fd_seek = () => 8, fd_write = () => 8;
