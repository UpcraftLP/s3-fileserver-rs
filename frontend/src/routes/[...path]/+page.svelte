<script lang="ts">
	import { ChevronRight, FileText, Folder, Layers } from 'svelte-feathers';
	import byteSize from 'byte-size';
	import type { PageData } from './$types';

	export let data: PageData;

	function trimPath(path: string) {
		const split = path.split('/');
		if (split[split.length - 1] === '') {
			split.pop();
		}
		return split.join('/');
	}

	$: path = data.path;
	$: trimmedPath = trimPath(path);
	$: splitTrimmedPath = trimmedPath.split('/');
	$: parentPath = trimmedPath.split('/').slice(0, -1).join('/');
</script>

<div class="flex flex-col justify-between h-screen p-4 gap-y-4 bg-stone-800 text-zinc-300">
	<div class="flex flex-col gap-y-4">
		<!-- Path -->
		<div class="w-full p-1 border border-zinc-600 bg-zinc-700">
			<a href="/" class="rounded-sm px-2 py-0.5 align-text-top hover:bg-zinc-600">
			<Layers class="inline w-6 h-6 align-top" /> Home
			</a>
			{#if path.length > 0}
				{#each trimmedPath.split('/') as pathFragment, i}
					<a
						href={`/${splitTrimmedPath.slice(0, i + 1).join('/')}`}
						class="rounded-sm p-0.5 pb-1 pr-2 text-center hover:bg-zinc-600"
					>
						<ChevronRight class="inline align-top" />{pathFragment}
					</a>
				{/each}
			{/if}
		</div>

		<!-- Content -->
		{#await data.data}
			<p>Loading...</p>
		{:then info}
			<ol
				class="flex flex-col w-full divide-y divide-zinc-700 child:w-full child:py-1 child:pl-1 child-hover:bg-zinc-900 child-focus:bg-blue-500"
			>
				{#if info.path.length > 1}
					<li itemid={`folder_parent`}>
						<a href={`/${parentPath}`} class="flex">
							<Folder class="inline" /><span class="mx-1">..</span>
						</a>
					</li>
				{/if}
				{#if info.folders}
					{#each info.folders as folder}
						<li itemid={`folder-${folder}`}>
							<a href={`/${info.path}${folder}`} class="flex">
								<Folder class="inline" /><span class="mx-1">{folder}</span>
							</a>
						</li>
					{/each}
				{/if}

				{#if info.files}
					{#each info.files as file}
						<li itemid={file.name}>
							<a
								href={file.download_url}
								class="grid grid-cols-3 min-w-7/12 child:min-w-max"
								target="_blank"
								data-sveltekit-preload-data="tap"
							>
								<span>
									<FileText class="inline" />
									<span>{file.name}</span>
								</span>
								<span class="invisible my-auto ml-auto text-sm italic text-zinc-500 lg:visible">
									{new Date(file.last_modified).toLocaleString()}
								</span>
								<span class="invisible justify-self-end sm:visible">{byteSize(file.size)}</span>
							</a>
						</li>
					{/each}
				{/if}
			</ol>
		{/await}
	</div>
</div>
