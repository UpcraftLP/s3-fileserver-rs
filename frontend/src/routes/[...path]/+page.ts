import type { PageLoad } from "./$types";
import { PUBLIC_API_URL } from '$env/static/public';

export const load: PageLoad = async ({ fetch, params: { path } }) => {

    let url = PUBLIC_API_URL;
    if (url.endsWith('/')) {
        url = url.substring(0, url.length - 1);
    }

    const data = fetch(`${url}/api/view/${path}`).then((res) => {
        if (res.ok) {
            return res.json();
        }
        throw new Error('Failed to load data');
    });
    return {
        path,
        data,
    }
};