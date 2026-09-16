import { readFileSync } from 'node:fs';
import { checkReleaseVersion } from './check_release_version.mjs';
import { collectReleaseArtifacts } from './release_artifacts.mjs';

/** Keep the release private until every installer has been uploaded successfully. */
export async function publishRelease({ github, repo, tag, artifactRoot, body }) {
  const { prerelease } = checkReleaseVersion(tag, { release: tag?.slice(1) });
  const assets = collectReleaseArtifacts(artifactRoot);
  const api = github.rest.repos;
  // Listing also finds drafts when resuming a failed publish job.
  const releases = await github.paginate(api.listReleases, { ...repo, per_page: 100 });
  let release = releases.find((item) => item.tag_name === tag);
  if (release && !release.draft) {
    throw new Error(`${tag} is already published. Existing releases will not be overwritten.`);
  }
  if (!release) {
    ({ data: release } = await api.createRelease({
      ...repo,
      tag_name: tag,
      name: `nooboard ${tag}`,
      body,
      draft: true,
      prerelease,
    }));
  }

  const releaseParams = { ...repo, release_id: release.id };
  const existing = await github.paginate(api.listReleaseAssets, {
    ...releaseParams,
    per_page: 100,
  });
  const names = new Set(assets.map((asset) => asset.name));
  if (existing.some((asset) => !names.has(asset.name))) {
    throw new Error(`${tag} draft contains unexpected assets. Review it before retrying.`);
  }
  for (const asset of assets) {
    const previous = existing.find((item) => item.name === asset.name);
    if (previous) {
      await api.deleteReleaseAsset({ ...repo, asset_id: previous.id });
    }
    await api.uploadReleaseAsset({
      ...releaseParams,
      name: asset.name,
      data: readFileSync(asset.path),
      headers: { 'content-type': 'application/octet-stream', 'content-length': asset.size },
    });
  }

  const uploaded = await github.paginate(api.listReleaseAssets, {
    ...releaseParams,
    per_page: 100,
  });
  if (
    uploaded.length !== assets.length ||
    assets.some(
      (asset) =>
        !uploaded.some(
          (item) =>
            item.name === asset.name && item.size === asset.size && item.state === 'uploaded',
        ),
    )
  ) {
    throw new Error('Uploaded installers are incomplete; the release remains a draft.');
  }

  const { data: published } = await api.updateRelease({
    ...releaseParams,
    name: `nooboard ${tag}`,
    body,
    draft: false,
    prerelease,
    make_latest: prerelease ? 'false' : 'legacy',
  });
  return published.html_url;
}
