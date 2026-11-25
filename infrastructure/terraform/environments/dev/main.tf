terraform {
  required_version = ">= 1.5"
  backend "local" {}
}

provider "aws" {
  region = var.aws_region
}

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

variable "environment" {
  type    = string
  default = "dev"
}

module "vpc" {
  source = "../modules/vpc"
  environment = var.environment
}

output "vpc_id" {
  value = module.vpc.vpc_id
}
