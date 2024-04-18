<script lang="ts">
	import type { PageData } from './$types';

	export let data: PageData;

	function trimPath(path: string) {
		const split = path.split('/');
		if (split[split.length - 1] === '') {
			split.pop();
		}
		split.pop();
		return split.join('/');
	}
</script>

{#await data.data}
	<p>Loading...</p>
	<h3>Path: {data.path || '/'}</h3>
{:then info}
	<div>
		<h3>Path: {info.path || '/'}</h3>

		<ul>
			{#if info.path.length > 1}
				<li itemid={`folder_parent`}><a href={`/${trimPath(info.path)}`}>..</a></li>
			{/if}
			{#if info.folders}
				{#each info.folders as folder}
					<li itemid={`folder-${folder}`}><a href={`/${info.path}${folder}`}>{folder}</a></li>
				{/each}
			{/if}
		</ul>

		{#if info.files}
			<ul>
				{#each info.files as file}
					<li itemid={file.name}><a href={file.download_url} target="_blank" data-sveltekit-preload-data="tap">{file.name}</a></li>
				{/each}
			</ul>
		{/if}
	</div>
{:catch error}
	<p style="color: red">{error.message}</p>
{/await}
