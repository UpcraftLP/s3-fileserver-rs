import type { PageLoad } from "./$types";
import { PUBLIC_API_URL } from '$env/static/public';

export const load: PageLoad = async ({ fetch, params: { path } }) => {

    const data = fetch(`${PUBLIC_API_URL}/api/view/${path}`).then((res) => {
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