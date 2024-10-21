import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import * as sqs from 'aws-cdk-lib/aws-sqs';
import * as sns from 'aws-cdk-lib/aws-sns';
import * as subscriptions from 'aws-cdk-lib/aws-sns-subscriptions';
import * as events from 'aws-cdk-lib/aws-events';
import * as targets from 'aws-cdk-lib/aws-events-targets';
import * as dynamodb from 'aws-cdk-lib/aws-dynamodb';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as s3 from 'aws-cdk-lib/aws-s3';
import { RustFunction } from 'cargo-lambda-cdk';
import { Architecture } from 'aws-cdk-lib/aws-lambda';

export class ShukFileHandlerStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

      // TODO: CLEAN UP PERMISSIONS
      // Create IAM role for Lambda
      const converterLambdaRole = new iam.Role(this, 'LambdaExecutionRole', {
        assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com'),
      });
      // Add necessary permissions to Lambda role
      converterLambdaRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('service-role/AWSLambdaBasicExecutionRole'));
      converterLambdaRole.addToPolicy(new iam.PolicyStatement({
        actions: ['mediaconvert:CreateJob', 's3:GetObject', 's3:PutObject', 'iam:PassRole'],
        resources: ['*'],
      }));

      // TODO: CLEAN UP PERMISSIONS
      // Create IAM role BackerUpper Lambda
      const backupLambdaRole = new iam.Role(this, 'backupLambdaRole', {
        assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com'),
      });
      // Add necessary permissions to Lambda role
      backupLambdaRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('service-role/AWSLambdaBasicExecutionRole'));
      backupLambdaRole.addToPolicy(new iam.PolicyStatement({
        actions: ['s3:GetObject', 's3:PutObject', 's3:PutObjectTagging'],
        resources: ['*'],
      }));

      // Event Bridge rule
      const shukUploadRule = new events.Rule(this, 'shukUploadRule', {
      eventPattern: {
        source: ['aws.s3'],
        detailType: ["Object Created"],
        detail: {
          bucket: {
            name: ["aws-darko-videos"]
          },
          object: {
            key: [{
              "anything-but": {
                "suffix": ".mov"
              }
            }]
          }
        }
      }
    });

    const movVideoUploadRule = new events.Rule(this, 'movVideoUploadRule',{
      eventPattern: {
        source: ['aws.s3'],
        detailType: ["Object Created"],
        detail: {
          bucket: {
            name: ["aws-darko-videos"],
          },
          object: {
            key: [{
              suffix: ".mov"
            }]
          }
        }
      }
    });

    // SNS Topic
    const uploadTopic = new sns.Topic(this, 'uploadTopic',{
      displayName: 'Shuk Upload Topic'
    });

    // Add target to SNS
    shukUploadRule.addTarget(new targets.SnsTopic(uploadTopic));

    // Rust Lambda Function - backup files
    const backupLambda = new RustFunction(this, 'backupLambda',{
      manifestPath: './lambdas/backup-files',
      architecture: Architecture.X86_64,
      memorySize: 128,
      timeout: cdk.Duration.minutes(2),
      role: backupLambdaRole
    });

    // SNS subscription
    uploadTopic.addSubscription(new subscriptions.LambdaSubscription(backupLambda));

    // SQS queue
    const metaDataQueue = new sqs.Queue(this, 'metaDataQueue');
    // SNS subscription
    uploadTopic.addSubscription(new subscriptions.SqsSubscription(metaDataQueue));
    
    // DynamoDB Table
    const metaDataTable = new dynamodb.Table(this, 'metaDataTabe', {
      tableName: 'metaDataTable',
      partitionKey: {
        name: 'file_id',
        type: dynamodb.AttributeType.STRING,
      },
      deletionProtection: false,
      removalPolicy: cdk.RemovalPolicy.DESTROY,
    });

    // Rust Lambda Function - Convert Mov files
    const convertMovLambda = new RustFunction(this, 'convertMovLambda',{
      manifestPath: './lambdas/convert-mov',
      architecture: Architecture.X86_64,
      memorySize: 128,
      timeout: cdk.Duration.minutes(3),
      role: converterLambdaRole
    });
    // Add Mov Target
    movVideoUploadRule.addTarget(new targets.LambdaFunction(convertMovLambda, {
      maxEventAge: cdk.Duration.hours(2),
      retryAttempts: 2,
      event: events.RuleTargetInput.fromObject({
        input_bucket: events.EventField.fromPath('$.detail.bucket.name'),
        input_key: events.EventField.fromPath('$.detail.object.key'),
        output_bucket: events.EventField.fromPath('$.detail.bucket.name'),
      }),
    }));
    // Rust Function for SQS to DynamoDB

  }
}
