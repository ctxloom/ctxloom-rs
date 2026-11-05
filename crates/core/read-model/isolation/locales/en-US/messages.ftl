# Messages for the crate's errors. Each id is a code, which begins with the
# crate's name; the variables are its args. An id ending -fix is the remedy of the one before.

isolation-no-runtime = { $runtime } was demanded and no container runtime is reachable here, so the run is refused rather than started unsandboxed
isolation-no-runtime-fix = install and start docker or podman, or declare runtime: host on an agent that should run unsandboxed; --degraded does not rescue a demanded container
isolation-ownership-mismatch = { $runtime } was demanded, but the reachable runtime offers only { $offered }; the other ownership is never substituted
isolation-ownership-mismatch-fix = declare the ownership this machine offers, or make the demanded one reachable; --degraded does not rescue it
isolation-image-unavailable = { $runtime } was demanded and the agent image can be neither found nor built, so the run is refused rather than started unsandboxed
isolation-image-unavailable-fix = run ctxloom container build, or point isolation_images at an image that exists; --degraded does not rescue it
