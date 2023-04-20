import { DuneBundle, DuneBundleTransaction, RpcBundle } from "./models";
import { S3, STS } from "aws-sdk";
import log from "./log";
import { Config } from "./config";
import { ethers } from "ethers";

export class S3Uploader {
  private bucketName: string;
  private externalId: string;
  private rolesToAssume: Array<string>;
  private s3: S3;

  constructor(config: Config) {
    this.bucketName = config.BUCKET_NAME;
    this.externalId = config.EXTERNAL_ID;
    this.rolesToAssume = config.ROLES_TO_ASSUME.split(',')
  }

  public async createS3() {
    log.debug(`Creating S3 instance`);
    const timestamp = new Date().getTime();
    let credentials = null;
    for (const role of this.rolesToAssume) {
      log.debug(`Assuming role ${role}`);
      const sts: STS = new STS({credentials})
      const auth = (await sts.assumeRole({
        RoleArn: role,
        RoleSessionName: `mevblocker-dune-sync-${timestamp}`,
        ExternalId: this.externalId
      }).promise()).Credentials;
      credentials = {
          accessKeyId: auth.AccessKeyId,
          secretAccessKey: auth.SecretAccessKey,
          sessionToken: auth.SessionToken
      }
    }

    this.s3 = new S3(credentials);
  }
  public async upload(bundle: RpcBundle, bundleId: string) {
    const duneBundle = convertBundle(bundle, bundleId);
    let retry = false;
    try {
      if (!this.s3) {
        await this.createS3();
      } else {
        // if we are using a cached s3 instance we may want to retry in case of failure
        retry = true
      }
      const params = {
        Bucket: this.bucketName,
        Key: `mevblocker_${Number(bundle.blockNumber)}_${bundleId}`,
        Body: JSON.stringify(duneBundle),
        ACL: 'bucket-owner-full-control'
      };
      log.debug(`Writing log: ${JSON.stringify(duneBundle)}`);
      const res = await this.s3.upload(params).promise();
      log.debug(`File Uploaded successfully ${res.Location}`);
    } catch (error) {
      log.error(`Unable to Upload the file: ${error}, retrying: ${retry}`);
      // Make sure we re-initialize the connection next time
      this.s3 = undefined;
      if (retry) {
        this.upload(bundle, bundleId)
      }
    }
  }
}

export function convertBundle(bundle: RpcBundle, bundleId: string): DuneBundle {
  return {
    blockNumber: Number(bundle.blockNumber),
    bundleId,
    transactions: bundle.txs.map((tx) => decodeTx(tx)),
  };
}

function decodeTx(tx: string): DuneBundleTransaction {
  const parsed = ethers.utils.parseTransaction(tx);
  return {
    nonce: parsed.nonce,
    maxFeePerGas: parsed.maxFeePerGas?.toString(),
    maxPriorityFeePerGas: parsed.maxPriorityFeePerGas?.toString(),
    gasPrice: parsed.gasPrice?.toString(),
    gasLimit: parsed.gasLimit.toString(),
    to: parsed.to,
    from: parsed.from,
    value: parsed.value.toString(),
    data: parsed.data,
    hash: parsed.hash,
  };
}
