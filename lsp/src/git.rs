use git2::Repository;

fn get_repository(path: &str) -> Option<Repository> {
    match Repository::open(path) {
        Ok(repo) => Some(repo),
        Err(_) => None,
    }
}

fn get_main_remote_url(repository: Repository) -> Option<String> {
    if let Ok(remote) = repository.find_remote("origin") {
        return remote.url().map(|url| transform_url(url.to_string()));
    }

    return match repository.remotes() {
        Ok(remotes) => repository
            .find_remote(remotes.get(0).unwrap())
            .unwrap()
            .url()
            .map(|url| transform_url(url.to_string())),
        Err(_) => None,
    };
}

fn transform_url(url: String) -> String {
    if url.starts_with("https://") {
        return url;
    }

    if let Some((_, rest)) = url.split_once('@') {
        if let Some((domain, path)) = rest.split_once(':') {
            return format!("https://{}/{}", domain, path);
        }
    }

    url.to_string()
}

pub fn get_repository_and_remote(path: &str) -> Option<String> {
    match get_repository(path) {
        Some(repository) => get_main_remote_url(repository),
        None => None,
    }
}
