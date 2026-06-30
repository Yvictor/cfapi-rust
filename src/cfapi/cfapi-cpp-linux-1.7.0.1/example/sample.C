
/*
** API test client program - Sample reads the requests from an input file. The input file
** have key-value pair instead of ctf tokens & each key value pair is separated by space/tab.
** Request Format is :  " COMMAND=value  ENUM_SRC_ID=value  SYMBOL_TICKER=value "
** Sample querySnap  :  " COMMAND=QuerySnap  ENUM_SRC_ID=558  SYMBOL_TICKER=IBM "
** Linux : make it with g++ -o sample sample.C -l pthread -L ../lib -l cfapi -l z -std=c++11 -I ../include/
*/


#ifndef DOXYGEN_SHOULD_SKIP_THIS

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <fcntl.h>
#include <ctype.h>
#include <cmath>
#include <map>
#include <vector>
#include <set>
#include <algorithm>
#define __STDC_FORMAT_MACROS 1
#include <inttypes.h>

/* required for socket */
#include <errno.h>
#include <mutex>
#include <thread>
#include <atomic>
std::thread send_thread;
std::thread::id send_thread_id;

#ifdef WIN32
#include <io.h>

#include <Windows.h>
#include <iostream>

#define SPRINTF sprintf_s

#else
#include <sys/types.h>
#include <sys/time.h>
#include <sys/types.h>
#include <unistd.h>

#define SPRINTF sprintf
#endif 

#include "cfapi.h"

#ifndef __useconds_t_defined
typedef unsigned long	useconds_t;
#endif


#define TRUE     1
#define FALSE    0


#if ( WINDOWS | WIN32 | WINNT )
#define SOCK_ERR	10000
#else
#define SOCK_ERR	-1
#endif

char remoteHost[32];
short remotePort;

void levelZero(cfapi::Session &); 
void printAuthStatus(cfapi::UserInfo& user);

std::atomic<bool> isSessionEstablished(false);
bool thread_active  = false;
std::atomic<bool> exitProgram(false);
bool sessionStarted       = false;

std::mutex writeMsgMutex;
std::mutex reqMapMutex;
std::map<int64_t, std::string> reqMap;


class ApiSessionHolder
{
	public:
		static ApiSessionHolder * createInstance(cfapi::Session &session)
		{
        		if(inst == NULL)
                		inst = new ApiSessionHolder(session);
			return inst;
		}

		void destroyInstance()
		{
			delete inst;
			inst = NULL;
		}

		static ApiSessionHolder *getInstance()
		{
        		return inst;
		}

		cfapi::Session & getApiSession()
		{
			return m_session;
		}

	private:
		ApiSessionHolder(cfapi::Session &session) : m_session(session)
		{
		}

		ApiSessionHolder(void);
		~ApiSessionHolder(void) {};
		ApiSessionHolder(const ApiSessionHolder &);
                ApiSessionHolder & operator=(const ApiSessionHolder &);

		cfapi::Session &m_session;
       		static ApiSessionHolder * inst;
};

ApiSessionHolder * ApiSessionHolder::inst = NULL;

struct ProgData
{
	unsigned long		mDataRate;	// Receive this # of Bps; 0 => unlimited
	int			mQuiet;		// boolean
	bool			mDebug;
	bool 			mMultithread;
	char			*mRemoteHost;
	unsigned short		mRemotePort;
	char			*mRemoteHost2;
	unsigned short		mRemotePort2;
	char			*mFileName;
	bool			mCompressedData;
	FILE 			*outFile;
	long 			mMaxUserThreads;
	long 			mMaxCspThreads;
	char 			*mUsername;
	char 			*mPassword;
	long 			mReadTimeout;
	long 			mStatsInterval;
	bool			mConflationIndicator;
	long 			mQueueSize;
	long 			mConflInterval;
	char            *mNatFile;
	long 			mConflType;
	long 			mJitThreshold;
	long 			mQueueThreshold;
	bool			mWatchlist;
	bool			mEarlySend;
	long            mWatchlistSize;
	long            mConnTimeout;
	std::vector<std::string> mPrimaryConn;
	std::vector<std::string> mBackupConn;
};

extern void		printHelp(void);
extern int 		readCommandLine(int argc, char **argv, struct ProgData *progdata);
extern void		*send_it();

struct ProgData	gProgData = { 0, 0, false, false, NULL, 0, NULL, 0, NULL, false, NULL, -1, -1, NULL, NULL, -1, -1, false, -1, -1, NULL, 0, -1, -1, false, false, -1, -1 };

std::map<std::string, int> CommandTokenMap{
	{ "COMMAND", 0 }, { "ENUM_SRC_ID", 1 }, { "SYMBOL_TICKER", 2 },
	{ "CUSIP", 3 },   { "SEDOL", 4 },  { "ISIN", 5 },
	{ "ENUM_SRC_UNDERLYING_ID", 6 }, { "SYMBOL_UNDERLYING_TICKER", 7 },
	{ "CONFLATION", 8}, { "CTF_TOKEN_NUM", 9}, { "SYMBOL_BLOOMBERG_TICKER", 10}, { "PRODUCT_ROOT", 11 },
	{ "CTF_TOKEN_NAME", 12}, { "SYMBOL_ESIGNAL_TICKER", 13}, { "DEPTH_TYPE", 14}, { "CONFLATION_INTERVAL", 15}, {"CTF_FILTER_ID", 16}, 
	{ "QUERY_REF_TAG", 17}, { "GETSOURCEPORTDETAILS", 18}
};

char filename[128];
char tbuf[4096 * 2];
FILE *testdata = NULL;


extern "C" {
	void sigproc(int param);

	void sigproc(int param)
	{
		if (!sessionStarted && !thread_active)
			exit(0);	//hard exit
		else
			exitProgram.exchange(true);	//clean exit
	}
}


void	printHelp(void)
{
	fprintf(stderr, "Usage: sample [-h] [-q] [-D] [-o output file] [-u userid] [-p password] [-M] [-U max_user_threads] [-C max_csp_threads] [-z] [-c] "
                    "[-r read timeout] [-s interval] [-c] [-Q queue_size] [-a conflation_interval] [-n NAT_file] [-t conflation_type] [-j JIT_threshold] [-w] [-e] [-W max_watchlist_size] [-f connection_timeout]"
					"-P primary_host_ip_address primary_port [-B backup_host_ip_address backup_port] input_command_file\n");
	fprintf(stderr, "-h:             Prints this help message\n");
	fprintf(stderr, "-q:             Quiet mode - don't print data\n");
	fprintf(stderr, "-D:             Debug mode - print API debug messages to stderr\n");
	fprintf(stderr, "-o:             Output file - write data here instead of stdout\n");
	fprintf(stderr, "-u:             userid for CSP\n");
	fprintf(stderr, "-p:             password for CSP\n");
	fprintf(stderr, "-M:             Use Multithreading mode in API \n\t\t[One thread per TCP connection to CSP for better performnce]\n");
	fprintf(stderr, "-U:             Max user-side threads API may create when in Multithreading mode\n\t\t[with -M option]\n");
	fprintf(stderr, "-C:             Max CSP-side threads API may create when in Multithreading mode\n\t\t[with -M option]\n");
	fprintf(stderr, "-z:             Turn off compression between API and CSP\n");
	fprintf(stderr, "-r:             read timeout in seconds \n\t\t[Default 5 seconds]\n");
	fprintf(stderr, "-s:             Get API statistics; specify interval in seconds\n");
	fprintf(stderr, "-c:             Request conflation indicator\n");
	fprintf(stderr, "-Q:             queue size in megabytes\n\t\t[Default 1 MB]\n");
	fprintf(stderr, "-a:             Conflation interval in milliseconds. Default is 1000 (ms)\n"); 
	fprintf(stderr, "-n:             NAT IP address mapping file (each line should contain <CSP IP addr>,<local IP addr>)\n");
	fprintf(stderr, "-t:             Conflation type. 1=Trade Safe(default); 2=Intervalized; 3=Just-In-Time (JIT); 4=Just-In-Time(JIT) Intervalized\n"); 
	fprintf(stderr, "-j:             JIT Conflation threshold. Buffer percent full to trigger conflation. 1-75 Default is 25.  Only valid with -t3 or -t4\n"); 
	fprintf(stderr, "-T:             Queue threshold. Buffer percent full to trigger CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD and CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD SessionEvents. 1-101 Default is 70.\n");   
	fprintf(stderr, "-w:             Use watchlist.\n"); 
	fprintf(stderr, "-W:             Max watchlist size.\n\t\t[Default 10000000(requests)]\n");
	fprintf(stderr, "-e:             Early send (send before Session start).\n"); 
	fprintf(stderr, "-f:             Connection timeout in seconds \n\t\t[Default 5 seconds]\n");
	fprintf(stderr, "-P:             Primary connection  with <IP addr><Port no>\n");
	fprintf(stderr, "-B:             Backup connection  with <IP addr><Port no>\n");
	fflush(stderr);
}

int readCommandLine(int argc, char **argv, struct ProgData *progdata)
{
	int	rc = 0, c;
	progdata->mDebug = false;
	progdata->mMultithread = false;
	progdata->mMaxUserThreads = -1;	//invalid value
	progdata->mMaxCspThreads = -1;	//invalid value
	progdata->mCompressedData = true;
	progdata->mReadTimeout = -1;	//invalid value
	progdata->mStatsInterval = -1;	//invalid value
	progdata->mConflationIndicator = false;
	progdata->mQueueSize = -1;
	progdata->mConflInterval = -1;
	progdata->mNatFile = NULL;
	progdata->mConflType = -1;
	progdata->mJitThreshold = -1;
	progdata->mQueueThreshold = -1;
	progdata->mWatchlist = false;
	progdata->mEarlySend = false;
	progdata->mWatchlistSize = -1;
	progdata->mConnTimeout = -1;	//invalid value

	progdata->mPrimaryConn.clear();
	progdata->mBackupConn.clear();

	std::set<std::string> primaryIPs;
	std::set<std::string> backupIPs;
	std::set<int> primaryPorts;
	std::set<int> backupPorts;
#ifdef WIN32

	if (argc < 4)
	{
		printHelp();
		return(0);
	}

	int i = 0;
	int option = 1;
	for (i = 1; i < argc - 1; ++i)
	{
		std::string arg = argv[i];

		if (arg == "-h") {
			printHelp();
			return(0);
		}

		if (arg == "-q") {
			progdata->mQuiet = 1;
			option++;
		}

		if (arg == "-D") {
			progdata->mDebug = true;
			option++;
		}

		if (arg == "-o") {
			fopen_s(&(progdata->outFile), argv[++i], "w+");
			option += 2;
		}

		if (arg == "-M") {
			progdata->mMultithread = true;
			option++;
		}

		if (arg == "-U") {
			progdata->mMaxUserThreads = atoi(argv[++i]);
			option += 2;
		}

		if (arg == "-C") {
			progdata->mMaxCspThreads = atoi(argv[++i]);
			option += 2;
		}

		if (arg == "-z") {
			progdata->mCompressedData = false;
			option++;
		}

		if (arg == "-u") {
			progdata->mUsername = argv[++i];
			option += 2;
		}

		if (arg == "-p") {
			progdata->mPassword = argv[++i];
			option += 2;
		}
		if (arg == "-r") {
			progdata->mReadTimeout = atoi(argv[++i]);
			option += 2;
		}
		if (arg == "-s") {
			progdata->mStatsInterval = atoi(argv[++i]);
			option += 2;
		}

		if (arg == "-c") {
			progdata->mConflationIndicator = true;
			option++;
		}

		if (arg == "-Q") {
			progdata->mQueueSize = atoi(argv[++i]);
			option += 2;
		}
		if (arg == "-a") {
			progdata->mConflInterval = atoi(argv[++i]);
			option += 2;
		}
		if (arg == "-n"){
			progdata->mNatFile = argv[++i];
			option+=2;
		}
		if (arg == "-t") {
			progdata->mConflType = atoi(argv[++i]);
			option += 2;
		}
		if (arg == "-j") {
			progdata->mJitThreshold = atoi(argv[++i]);
			option += 2;
		}
		if (arg == "-T") {
			progdata->mQueueThreshold = atoi(argv[++i]);
			option += 2;
		}
		if (arg == "-w") {
			progdata->mWatchlist = true;
			option++;
		}
		if (arg == "-W") {
			progdata->mWatchlistSize = atoi(argv[++i]);
			option += 2;
		}
		if (arg == "-e") {
			progdata->mEarlySend = true;
			option++;
		}
		if (arg == "-f") {
			progdata->mConnTimeout = atoi(argv[++i]);
			option += 2;
		}
		if (arg == "-P") 
		{
			std::string ip  = argv[++i];
			if (i >= argc-1)
			{
				printf(" Invalid Primary entry ip/port missing \n");
				printHelp();
				return(0);
			}

			std::string port = argv[++i];
			std::string hoststring = ip + ":" + port;

			//if (primaryIPs.find(ip) != primaryIPs.end() || // ip can be same for CM case
			if(primaryPorts.find(atoi(port.c_str())) != primaryPorts.end())
			{
				printf("Primary port (%s) repeated \n",  port.c_str());
				printHelp();
				return(0);
			}

			primaryIPs.insert(ip);
			primaryPorts.insert(atoi(port.c_str()));

			if(std::find(progdata->mPrimaryConn.begin(), progdata->mPrimaryConn.end(), hoststring) == progdata->mPrimaryConn.end())
			{
				progdata->mPrimaryConn.push_back(hoststring);
				option += 3;
			}
			else
			{
				printHelp();
				return(0);
			}
		}
		if (arg == "-B") {
			std::string ip = argv[++i];
			if (i >= argc-1)
			{
				printf(" Invalid Backup entry ip/port missing \n");
				printHelp();
				return(0);
			}
			std::string port = argv[++i];
			std::string hoststring = ip + ":" + port;

			if(backupPorts.find(atoi(port.c_str())) != backupPorts.end())
			{
				printf("Backup port (%s) repeated \n",  port.c_str());	
				printHelp();
				return(0);
			}
			else if (primaryIPs.find(ip) != primaryIPs.end())
			{
				printf("Backup ip(%s) should be different from primary IPs \n", ip.c_str());
				printHelp();
				return(0);
			}

			backupIPs.insert(ip);
			backupPorts.insert(atoi(port.c_str()));

			if (std::find(progdata->mBackupConn.begin(), progdata->mBackupConn.end(), hoststring) == progdata->mBackupConn.end())
			{
				progdata->mBackupConn.push_back(hoststring);
				option += 3;
			}
			else
			{
				printHelp();
				return(0);
			}
		}
		//TODO default			
	}

	int optind = option;
#else
	
	std::string ip = "";
	std::string port = "";
	std::string hoststring = "";
	while ((rc >= 0) && ((c = getopt(argc, argv, "hqDo:MU:C:zu:p:r:s:cQ:a:n:t:j:T:weP:B:W:f:")) > 0))
	{
		switch ((char)c)
		{

		case 'h':
			printHelp();
			return(0);
			break;

		case 'q':
			progdata->mQuiet = 1;
			break;

		case 'D':
			progdata->mDebug = true;
			break;

		case 'o':
			progdata->outFile = fopen(optarg, "w");
			break;

		case 'M':
			progdata->mMultithread = true;
			break;

		case 'U':
			progdata->mMaxUserThreads = atoi(optarg);
			break;

		case 'C':
			progdata->mMaxCspThreads = atoi(optarg);
			break;

		case 'z':
			progdata->mCompressedData = false;
			break;

		case 'u':
			progdata->mUsername = optarg;
			break;

		case 'p':
			progdata->mPassword = optarg;
			break;

		case 'r':
			progdata->mReadTimeout = atoi(optarg);
			break;

		case 's':
			progdata->mStatsInterval = atoi(optarg);
			break;

		case 'c':
			progdata->mConflationIndicator = true;
			break;

		case 'Q':
			progdata->mQueueSize = atoi(optarg);
			break;

		case 'a':
			progdata->mConflInterval = atoi(optarg);
			break;

		case 'n':
			progdata->mNatFile = optarg;
			break;

		case 't':
			progdata->mConflType = atoi(optarg);
			break;

		case 'j':
			progdata->mJitThreshold = atoi(optarg);
			break;

		case 'T':
			progdata->mQueueThreshold = atoi(optarg);
			break;

		case 'w':
			progdata->mWatchlist = true;
			break;

		case 'W':
			progdata->mWatchlistSize = atoi(optarg);
			break;

		case 'e':
			progdata->mEarlySend = true;
			break;

		case 'f':
			progdata->mConnTimeout = atoi(optarg);
			break;

		case 'P':
			ip = optarg;
			if (optind >= argc-1)
			{
				printf(" Invalid Primary entry ip/port missing \n");
				printHelp();
				return(-1);
			}
			port = argv[optind++];
			hoststring = ip + ":" + port;

			if (primaryPorts.find(atoi(port.c_str())) != primaryPorts.end())
			{
				printf("Primary  port  (%s) repeated \n",  port.c_str());
				printHelp();
				return(-1);
			}

			primaryIPs.insert(ip);
			primaryPorts.insert(atoi(port.c_str()));

			if (std::find(progdata->mPrimaryConn.begin(), progdata->mPrimaryConn.end(), hoststring) == progdata->mPrimaryConn.end())
			{
				progdata->mPrimaryConn.push_back(hoststring);
				//option += 3;
			}
			else
			{
				printHelp();
				return(-1);
			}
			break;

		case 'B':
			ip = optarg;
			if (optind >= argc -1)
			{
				printf(" Invalid Backup entry ip/port missing \n");
				printHelp();
				return(0);
			}

			port = argv[optind++];
			hoststring = ip + ":" + port;

			if ( backupPorts.find(atoi(port.c_str())) != backupPorts.end())
			{
				printf("Backup port (%s) repeated \n",  port.c_str());
				printHelp();
				return(-1);
			}

			backupIPs.insert(ip);
			backupPorts.insert(atoi(port.c_str()));

			if (std::find(progdata->mBackupConn.begin(), progdata->mBackupConn.end(), hoststring) == progdata->mBackupConn.end())
			{
				progdata->mBackupConn.push_back(hoststring);
				//option += 3;
			}
			else
			{
				printHelp();
				return(-1);
			}
			break;

		default:
			fprintf(stderr, "ERROR: Invalid command option\n");
			fflush(stderr);
			printHelp();
			return(-1);
			break;
		}
	}
#endif // WIN32

	if (!progdata->outFile)
		progdata->outFile = stdout;

	if ( argc - optind == 1) 
	{

#ifdef WIN32
		progdata->mFileName = (char *)malloc(strlen(argv[optind]) + 1);
		strcpy_s(progdata->mFileName, strlen(argv[optind]) + 1, argv[optind]);
		optind++;
		rc = optind;
#else
		progdata->mFileName = (char *)malloc(strlen(argv[optind]) + 1);
		strcpy(progdata->mFileName, argv[optind++]);
		rc = optind;
#endif

	}
	else
	{
		printf("ERROR: Invalid argument count\n");
		printHelp();
		rc = -1;
	}

	return(rc);
}

//Extra logic to ensure string version of double roughly matches existing CSP CTF output
//Force max width of 15, round and remove trailing 0s
void formatDoubleAsString(double value, char *valueStr, int valueStrLen)
{
	const int MAXWIDTH = 16;	//15 digits + the decimal point itself
	snprintf(valueStr, valueStrLen, "%.9f", value);
	int width = strlen(valueStr);
	if (width > MAXWIDTH)
	{
		//first round...
		char *decimal = strchr(valueStr, '.');
		double round = 5.0 / pow(10.0, valueStr + MAXWIDTH - decimal);
		snprintf(valueStr, valueStrLen, "%.9f", value + round);

		//...then truncate
		width = MAXWIDTH;
	}

	// remove trailing zeros
	// and decimal if there are no zeros
	char *s = valueStr + width;
	while (*(s - 1) == '0')
		s--;

	if (*(s - 1) == '.')
		s--;
	*s = '\0';
}


/*
 * Simple event handler to print API statistics
 */

class MyStatsEventHandler : public cfapi::StatisticsEventHandler
{
	void onStatisticsEvent(const cfapi::StatisticsEvent& event)
	{
		printf("\nMSGS_IN --> %" PRIu64 "; MSGS_OUT --> %" PRIu64 "; NET_MSGS_OUT --> %" PRIu64 "; DROP --> %" PRIu64 "; CSP_DROP --> %" PRIu64 "; PCT_FULL=%" PRIu64 "/%" PRIu64 "; IN_MSGS/SEC(100ms)=%" PRIu64 "/%" PRIu64 " IN_MSGS/SEC(1sec)=%" PRIu64 "/%" PRIu64 "; OUT_MSGS/SEC(100ms)=%" PRIu64 "/%" PRIu64 " OUT_MSGS/SEC(1sec)=%" PRIu64 "/%" PRIu64 "; NET_OUT_MSGS/SEC(100ms)=%" PRIu64 "/%" PRIu64 " NET_OUT_MSGS/SEC(1sec)=%" PRIu64 "/%" PRIu64 "\n\n", event.getStat(cfapi::StatisticsEvent::MSGS_IN), event.getStat(cfapi::StatisticsEvent::MSGS_OUT), event.getStat(cfapi::StatisticsEvent::NET_MSGS_OUT), event.getStat(cfapi::StatisticsEvent::DROP), event.getStat(cfapi::StatisticsEvent::CSP_DROP), event.getStat(cfapi::StatisticsEvent::PCT_FULL), event.getStat(cfapi::StatisticsEvent::PEAK_PCT_FULL),  event.getStat(cfapi::StatisticsEvent::IN_MSGS_SEC_100MS), event.getStat(cfapi::StatisticsEvent::PEAK_IN_MSGS_SEC_100MS), event.getStat(cfapi::StatisticsEvent::IN_MSGS_SEC), event.getStat(cfapi::StatisticsEvent::PEAK_IN_MSGS_SEC), event.getStat(cfapi::StatisticsEvent::OUT_MSGS_SEC_100MS), event.getStat(cfapi::StatisticsEvent::PEAK_OUT_MSGS_SEC_100MS), event.getStat(cfapi::StatisticsEvent::OUT_MSGS_SEC), event.getStat(cfapi::StatisticsEvent::PEAK_OUT_MSGS_SEC), event.getStat(cfapi::StatisticsEvent::NET_OUT_MSGS_SEC_100MS), event.getStat(cfapi::StatisticsEvent::PEAK_NET_OUT_MSGS_SEC_100MS), event.getStat(cfapi::StatisticsEvent::NET_OUT_MSGS_SEC), event.getStat(cfapi::StatisticsEvent::PEAK_NET_OUT_MSGS_SEC));
	}
};


/*
*  * Simple event handler to print status of login
*   */
class MyUserEventHandler : public cfapi::UserEventHandler
{
	void onUserEvent(const cfapi::UserEvent& event)
	{
		cfapi::UserEvent::Types eventType = event.getType();
		if (eventType == cfapi::UserEvent::AUTHORIZATION_FAILURE)
		{
			printf("login failed: return code = %d: %s\n", event.getRetCode(), event.getRetCodeString().c_str());
		}

		if (gProgData.mDebug)
			printAuthStatus(event.getSession().getUserInfo());
		
	}
};


/*
*  * Simple event handler to print status of session
*   */
class MySessionEventHandler : public cfapi::SessionEventHandler
{
	void onSessionEvent(const cfapi::SessionEvent& event)
	{
		cfapi::SessionEvent::Types eventType = event.getType();
		if (eventType == cfapi::SessionEvent::CFAPI_SESSION_UNAVAILABLE)
		{
			printf("session unavailable\n");
		}
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_ESTABLISHED)
		{
			printf("SESSION_ESTABLISHED\n");
			isSessionEstablished.exchange(true);

		}
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_RECOVERY)
		{
			printf("SESSION_RECOVERY\n");
		}
		else if (eventType == cfapi::SessionEvent::CFAPI_CDD_LOADED)
		{
			printf("CFAPI_CDD_LOADED; version=%s\n", event.getCddVersion().c_str());
		}
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_RECOVERY_SOURCES)
		{
			int sourceID = event.getSourceID();
			printf("CFAPI_SESSION_RECOVERY_SOURCES  sourceID = %d \n", sourceID);
		}
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_AVAILABLE_ALLSOURCES)
		{
			int sourceID = event.getSourceID();
			printf("CFAPI_SESSION_AVAILABLE_ALLSOURCES  sourceID = %d \n", sourceID);
		}
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_AVAILABLE_SOURCES)
		{
			int sourceID = event.getSourceID();
			printf("CFAPI_SESSION_AVAILABLE_SOURCES  sourceID = %d \n", sourceID);
		}
        else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD)
        {
            printf("SESSION_RECEIVE_QUEUE_ABOVE_THRESHOLD (%d)\n", event.getQueueDepth());
        }
        else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD)
        {
            printf("SESSION_RECEIVE_QUEUE_BELOW_THRESHOLD (%d)\n", event.getQueueDepth());
        }
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_JIT_START_CONFLATING)
			printf("CFAPI_SESSION_JIT_START_CONFLATING\n");
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_JIT_STOP_CONFLATING)
			printf("CFAPI_SESSION_JIT_STOP_CONFLATING\n");
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_SOURCE_ADDED)
		{
			int sourceID = event.getSourceID();
			printf("CFAPI_SESSION_SOURCE_ADDED (%d)\n", sourceID);
		}
		else if (eventType == cfapi::SessionEvent::CFAPI_SESSION_SOURCE_REMOVED)
		{
			int sourceID = event.getSourceID();
			printf("CFAPI_SESSION_SOURCE_REMOVED (%d)\n", sourceID);
		}
		else
			printf("Unknown session state\n");
	}
};

class MyMessageEventHandler : public cfapi::MessageEventHandler
{
	void onMessageEvent(const cfapi::MessageEvent &event)
	{
		char valueStr[1000];
		std::string strValue;

		char datetimeStr[100];
		cfapi::DateTime datetime;

		char msgOutStr[64000];	//News fields could be long
		std::string outMsg;

		if ((event.getType() == cfapi::MessageEvent::STATUS) || (event.getType() == cfapi::MessageEvent::IMAGE_COMPLETE))
		{
			reqMapMutex.lock();
			std::string command(reqMap[event.getTag()]);
			if (!gProgData.mWatchlist)	//In watchlist mode, we may see this tag again on a reconnect
				reqMap.erase(event.getTag());
			reqMapMutex.unlock();

			printf("Status code = %d (%s) for tag %llu (%s)\n", event.getStatusCode(), 
				     event.getStatusString().c_str(), (long long)event.getTag(), command.c_str());
		}
		else if (gProgData.mConflationIndicator)
		{
			const char *tmpStr;
			switch (event.isConflatable())
			{
				case 0:
					tmpStr = "not ";
					break;
				case 1:
					tmpStr = "";
					break;
				default:
					tmpStr = "not known if ";
			}
			printf("This message is %sconflatable\n", tmpStr);
		}
		//Only add if each is present
		int perm = event.getPermission();
		if (perm && !gProgData.mQuiet) {
			SPRINTF(msgOutStr, "PERMISSION(3)=%d\n", perm);
			outMsg.append(msgOutStr);
		}
		int src = event.getSource();
		if (src && !gProgData.mQuiet) {
			SPRINTF(msgOutStr, "ENUM.SRC.ID(4)=%d\n", src);
			outMsg.append(msgOutStr);
		}
		if (!event.getSymbol().empty() && !gProgData.mQuiet) {
			SPRINTF(msgOutStr, "SYMBOL.TICKER(5)=%s\n", event.getSymbol().c_str());
			outMsg.append(msgOutStr);
		}
		if (!gProgData.mQuiet)
		{	//May be more than one alternate index associated with this message
			for (size_t i = 0; i < event.getNumberofAlternateIndexes(); i++)
			{
				SPRINTF(msgOutStr, "%s(%d)=%s\n", event.getAlternateIndexTokenName(i).c_str(), event.getAlternateIndexTokenNumber(i), event.getAlternateIndexValue(i).c_str());
				outMsg.append(msgOutStr);
			}
		}
		if ((event.getType() == cfapi::MessageEvent::REFRESH) && !gProgData.mQuiet) {
			SPRINTF(msgOutStr, "REFRESH(24)=1\n");
			outMsg.append(msgOutStr);
		}

		cfapi::MessageReader &rdr = event.getReader();
		while (rdr.next() != cfapi::MessageReader::END_OF_MESSAGE)
		{
			switch (rdr.getValueType())
			{
			case cfapi::INT64:
				if (!gProgData.mQuiet) {
					SPRINTF(msgOutStr, "%s(%d)=%lld\n", rdr.getTokenName().c_str(),
						rdr.getTokenNumber(), (long long)rdr.getValueAsInteger());
					outMsg.append(msgOutStr);
				}
				break;

			case cfapi::STRING:
				if (!gProgData.mQuiet) {
					SPRINTF(msgOutStr, "%s(%d)=%s\n", rdr.getTokenName().c_str(),
						rdr.getTokenNumber(), rdr.getValueAsString().c_str());
					outMsg.append(msgOutStr);
				}
					break;

				case cfapi::DOUBLE:
					if (!gProgData.mQuiet) {
						formatDoubleAsString(rdr.getValueAsDouble(), valueStr, sizeof(valueStr));
						SPRINTF(msgOutStr, "%s(%d)=%s\n", rdr.getTokenName().c_str(),
							rdr.getTokenNumber(), valueStr);
						outMsg.append(msgOutStr);
					}
					break;

				case cfapi::DATETIME:
					if (!gProgData.mQuiet) {
						datetime = rdr.getValueAsDateTime();
						formatDoubleAsString(rdr.getValueAsDouble(), valueStr, sizeof(valueStr));
						SPRINTF(datetimeStr, "%d-%02d-%02d %02d:%02d:%02d.%06d UTC", 
							    datetime.date().year(), datetime.date().month(), datetime.date().day(),
							    datetime.time().hour(), datetime.time().minute(), datetime.time().second(),
							    (datetime.time().millisecond() * 1000) + datetime.time().microsecond());
						SPRINTF(msgOutStr, "%s(%d)=%s(%s)\n", rdr.getTokenName().c_str(),
							rdr.getTokenNumber(), datetimeStr, valueStr);
						outMsg.append(msgOutStr);
					}
					break;

				case cfapi::UNKNOWN:
				default:
					if (!gProgData.mQuiet) {
						SPRINTF(valueStr, "%s", "UNKNOWN value type, cannot decode");
						SPRINTF(msgOutStr, "%s(%d)=%s\n", rdr.getTokenName().c_str(),
							rdr.getTokenNumber(), valueStr);
						outMsg.append(msgOutStr);
					}
			}

		}
		if (!gProgData.mQuiet)
		{
			outMsg.append("<ETX>\r\n");
			writeMsgMutex.lock();
			fprintf(gProgData.outFile, "%s", outMsg.c_str());
			writeMsgMutex.unlock();
			fflush(gProgData.outFile);
		}
	}
};

void printAuthStatus(cfapi::UserInfo& user)
{
	const char *stateStr = NULL;

	cfapi::UserInfo::States state = user.getState();
	switch (state)
	{
	case cfapi::UserInfo::NOT_AUTHENTICATED:
		stateStr = "NOT_AUTHENTICATED";
		break;
	case cfapi::UserInfo::AUTHENTICATED:
		stateStr = "AUTHENTICATED";
		break;
	case cfapi::UserInfo::PARTIALLY_AUTHENTICATED:
		stateStr = "PARTIALLY_AUTHENTICATED";
		break;

	}
	printf("Authorization status = %s\n", stateStr);
	fflush(stdout);
}


//Assume format of each line of file is:
// <CSP IP>,<local IP>
 
void parseNatFile(cfapi::ConnectionConfig &connectionConfig, char *filename)
{
	char nbuf[1024 * 2];
	size_t len = 0;
	char *comma = NULL;
	char *str = NULL;
	FILE *natdata = NULL;

#ifdef WIN32
	errno_t err = fopen_s(&natdata, filename, "r");
#else
	natdata = fopen(filename,"rb");
#endif
	
	if (natdata)
	{
		while (true)
		{
			str = fgets(nbuf,sizeof(nbuf),natdata);
			if (!str)   //EOF
			{
				break;  //done
			}	

			if(isspace(nbuf[0]) || nbuf[0] == '#' )
				continue;
			len=strlen(nbuf);

			if (nbuf[len-1] == '\n')    //strip newline
				nbuf[len-1] = '\0';

			comma = strchr(nbuf, ',');
			if (!comma)
			{
				printf("Invalid line (missing comma): %s\n", nbuf);
				continue;
			}
			*comma = '\0';
			char *ipCsp = nbuf;
			char *ipClient = comma + 1;
			connectionConfig.addNATPair(ipCsp, ipClient);
		}
		fclose(natdata);
	}
	else
		printf("Error opening %s\n", filename);
}

int main(int argc, char** argv)
{
	int	rc;
	char username[25];
	char password[25];

	signal(SIGINT, sigproc);

	printf("\n use ctrl-c to quit\n");

	if ((rc = readCommandLine(argc, argv, &gProgData)) <= 0)
	{
		if (gProgData.outFile)
			fclose(gProgData.outFile);
		return(-rc);
	}

#ifdef WIN32
	strcpy_s(filename, gProgData.mFileName);
#else
	strcpy(filename, gProgData.mFileName);
#endif

	if (!gProgData.mUsername)
	{
		printf("\nEnter userid: ");
		fgets(username, sizeof(username), stdin);
		//Strip trailing newline
		if (username[strlen(username) - 1] == '\n')
			username[strlen(username) - 1] = '\0';
	}
	else
	{

		strncpy(username, gProgData.mUsername, sizeof(username));
		username[sizeof(username) - 1] = '\0';

	}

	if (!gProgData.mPassword)
	{
		printf("\nEnter password: ");
		fgets(password, sizeof(password), stdin);
		if (password[strlen(password) - 1] == '\n')
			password[strlen(password) - 1] = '\0';
	}
	else
	{

		strncpy(password, gProgData.mPassword, sizeof(password));
		password[sizeof(password) - 1] = '\0';

	}

	MyUserEventHandler userEventHandler;
	MySessionEventHandler sessionHandler;
	cfapi::APIFactory::getInstance()->initialize("sample", "1.0.0", gProgData.mDebug, "cfapilog");
	cfapi::UserInfo &primaryUser = cfapi::APIFactory::getInstance()->createUserInfo(username, password, userEventHandler);
	cfapi::Session &session = cfapi::APIFactory::getInstance()->createSession(primaryUser, sessionHandler);

	// In this example program we are using the ApiSessionHolder Class defined above
	// This is not mandatory - you are welcome to use the session reference on it's own
	// If you want you can pass session directly into a thread that might send requests 
	// to the CSP 
	// It is shown here as an example on how to have a class that will allow the session reference
	// to be easily accessible to all areas of your program
	ApiSessionHolder *api_session_holder = ApiSessionHolder::createInstance(session);

	if (gProgData.mDebug)
		printAuthStatus(primaryUser);

	cfapi::SessionConfig &sessionConfig = session.getSessionConfig();
	if (gProgData.mMultithread)
	{
		sessionConfig.set(cfapi::SessionConfig::MULTITHREADED_API_CONNECTIONS_BOOL, true);
		if (gProgData.mMaxUserThreads != -1)
			sessionConfig.set(cfapi::SessionConfig::MAX_USER_THREADS_LONG, gProgData.mMaxUserThreads);
		if (gProgData.mMaxCspThreads != -1)
			sessionConfig.set(cfapi::SessionConfig::MAX_CSP_THREADS_LONG, gProgData.mMaxCspThreads);
	}
	if (gProgData.mWatchlist)
		sessionConfig.set(cfapi::SessionConfig::WATCHLIST_BOOL, true);

	if (gProgData.mWatchlistSize != -1)
	{
		cfapi::SessionConfig &sessionConfig = session.getSessionConfig();
		sessionConfig.set(cfapi::SessionConfig::MAX_WATCHLIST_SIZE_LONG, gProgData.mWatchlistSize);
	}

	cfapi::ConnectionConfig &connectionConfig = session.getConnectionConfig();
	connectionConfig.set(cfapi::HostConfig::CONFLATION_INDICATOR_BOOL, gProgData.mConflationIndicator);

	if (gProgData.mNatFile)
		parseNatFile(connectionConfig, gProgData.mNatFile);

	if (gProgData.mConflInterval != -1)
		connectionConfig.set(cfapi::HostConfig::CONFLATION_INTERVAL_LONG, gProgData.mConflInterval);
	if (gProgData.mConflType > -1)
		connectionConfig.set(cfapi::HostConfig::CONFLATION_TYPE_LONG, gProgData.mConflType);
	if (gProgData.mJitThreshold > -1)
	{
		if (gProgData.mConflType != 3 && gProgData.mConflType !=4)
			printf("WARN: set JIT threshold with non-JIT conflation; threshold will be ignored\n");
		connectionConfig.set(cfapi::HostConfig::JIT_CONFLATION_THRESHOLD_PERCENT_LONG, gProgData.mJitThreshold);
	}
	if (gProgData.mQueueThreshold > -1)
	{
		sessionConfig.set(cfapi::SessionConfig::QUEUE_DEPTH_THRESHOLD_PERCENT_LONG, gProgData.mQueueThreshold);
	}

	//connectionConfig.set(cfapi::HostConfig::CONNECTION_RETRY_LIMIT_LONG, (long)1);
	if (gProgData.mConnTimeout > -1)
		connectionConfig.set(cfapi::HostConfig::CONNECTION_TIMEOUT_LONG, gProgData.mConnTimeout);

	if (gProgData.mReadTimeout > -1)
		connectionConfig.set(cfapi::HostConfig::READ_TIMEOUT_LONG, gProgData.mReadTimeout);

	if (gProgData.mQueueSize != -1)
		connectionConfig.set(cfapi::HostConfig::QUEUE_SIZE_LONG, gProgData.mQueueSize);

	MyStatsEventHandler statsEH;
	if (gProgData.mStatsInterval > 0)
		session.registerStatisticsEventHandler(&statsEH, gProgData.mStatsInterval);

	//CEF changes
	if (!gProgData.mPrimaryConn.empty())
	{
		std::vector<std::string>::iterator it = gProgData.mPrimaryConn.begin();
		for (; it != gProgData.mPrimaryConn.end(); it++)
		{
			cfapi::HostConfig &hostConfig = connectionConfig.getHostConfig(*it);
			hostConfig.set(cfapi::HostConfig::BACKUP_BOOL, false);
			hostConfig.set(cfapi::HostConfig::COMPRESSION_BOOL, gProgData.mCompressedData);
		}
	}
	else // Primary conn not specified
	{
		printf(" Error - Primary connection not specified \n");
		printHelp();
		return 0;
	}

	if (!gProgData.mBackupConn.empty())  // backup server
	{
		if (gProgData.mBackupConn.size() != gProgData.mPrimaryConn.size())
		{
			printf("Error - The no of backup connections should be same as the primary connections\n");
			printHelp();
			return 0;
		}

		std::vector<std::string>::iterator it = gProgData.mBackupConn.begin();
		for (; it != gProgData.mBackupConn.end(); it++)
		{
			cfapi::HostConfig &hostConfig = connectionConfig.getHostConfig(*it);
			hostConfig.set(cfapi::HostConfig::BACKUP_BOOL, true);
			hostConfig.set(cfapi::HostConfig::COMPRESSION_BOOL, gProgData.mCompressedData);
		}
	}


	MyMessageEventHandler responseEH;
	session.registerMessageEventHandler(&responseEH);	//register to get called when messages received

	if (gProgData.mEarlySend)	
		levelZero(session);	//optionally send requests to API before session established (but after all other initalization calls)
	std::string failReason;
	bool ret = session.start(failReason);
	if (!ret)
	{
		printf("session could not be established: %s\n", failReason.c_str());
	}
	else
	{
		sessionStarted = true;

		if (!gProgData.mEarlySend )
			levelZero(session);	//normally, send requests after session has been established

		while (!exitProgram) {
#ifdef WIN32
			std::this_thread::sleep_for(std::chrono::seconds(2));
#else
			sleep(1);	
#endif 
		}
		printf("ctrl-c pressed to quit\n");
		printf("Stopping API session\n");
		session.stop();
	}

	ApiSessionHolder::getInstance()->destroyInstance();

	cfapi::APIFactory::getInstance()->destroySession(session);
	cfapi::APIFactory::getInstance()->destroyUserInfo(primaryUser);
	cfapi::APIFactory::getInstance()->uninitialize();
	
	free(gProgData.mRemoteHost);
	free(gProgData.mRemoteHost2);
	free(gProgData.mFileName);

	fclose(gProgData.outFile);

}



void levelZero(cfapi::Session &session)
{
	try
	{
		thread_active=true;
		send_thread = std::thread(send_it);
		send_thread_id = send_thread.get_id();

		send_thread.join();
		thread_active=false;
	}
	catch (...)
	{
		printf("Exception thrown when thread is terminated\n");
	}
}

int getTokenID(std::string cmd)
{
	std::transform(cmd.begin(), cmd.end(), cmd.begin(), ::toupper);

	std::map<std::string, int>::iterator itr = CommandTokenMap.find(cmd);
	if (itr != CommandTokenMap.end())
		return itr->second;
	else
	{
		//printf("Invalid token in the request: %s\n", cmd.c_str());
		return -1;
	}

}

void *send_it()
{
	size_t len;

	//if (gProgData.mDebug)
		//printAuthStatus(session.getUserInfo());

#ifdef WIN32
	errno_t err = fopen_s(&testdata, filename, "r");
#else
	testdata = fopen(filename, "rb");
#endif

	if (!testdata)
	{
#ifdef WIN32
		printf("Error Opening %s: %d \n", filename, err);	
#else
		printf( "Error Opening %s: %s \n", filename, strerror(errno));		
#endif
	}

	ApiSessionHolder *api_session_holder = ApiSessionHolder::getInstance();
	if(!api_session_holder)
	{
		printf("ERROR --> ApiSessionHolder not created");
		exit(1);
	}
	
	cfapi::Session &session = api_session_holder->getApiSession();
	cfapi::Request &request = session.createRequest();

	if (testdata)
	{
		while (fgets(tbuf, sizeof(tbuf), testdata))
		{
			if (isspace(tbuf[0]) || tbuf[0] == '#')
				continue;

			len = strlen(tbuf);

			if (tbuf[len - 1] == '\n')	//strip newline
				tbuf[len - 1] = '\0';

			std::string origbuf(tbuf);

			//Parse message, convert to API request
			request.clearRequest();
			std::size_t eq;
			std::string val;	
			std::string cmd;
			std::string remain(tbuf);
			
			std::string delims;
			bool badMsg = false;
			bool quotedVal = false;
			bool queueFull = false;
			int64_t reqHandle = 0;
			int start_of_val = 0;
			int cmdStart = 0;
			while (!remain.empty())
			{

				eq = remain.find("=");
				if (eq == std::string::npos)
				{
					badMsg = true;
					printf("Missing '=': %s\n", remain.c_str());
					break;
				}
				delims.assign(" \t");
				cmdStart = remain.find_first_not_of(delims);
				cmd = remain.substr(cmdStart, eq - cmdStart);
				val = remain.substr(eq + 1);
				
				if (val[0] == '"')
				{
					delims.assign("\"");
					quotedVal=true;
					start_of_val = 1;
				}
				else
				{
					quotedVal=false;
					start_of_val = 0;
				}

				int end_of_val = val.find_first_of(delims, start_of_val);
				if (std::string::npos == end_of_val)
					if (quotedVal)
					{
						badMsg = true;
						printf("Missing end-quote: %s\n", val.c_str());
						break;
					}
					else
						remain.clear();
				else
				{
					val = val.substr(start_of_val, end_of_val - start_of_val);
					remain = remain.substr(eq + 1 + end_of_val + 1);
				}

				std::string req;
				int toknum = getTokenID(cmd);
				int ctfid = 0;

				//printf("cmd=%s; toknum=%d; val=%s\n", cmd.c_str(), toknum, val.c_str());
				switch (toknum)
				{
				case 0:	//command
					req = val;
					std::transform(req.begin(), req.end(), req.begin(), ::toupper);
					//printf("val: %s; req=%s\n", val, req.c_str());

					if(req.compare("SUBSCRIBE") ==  0 )
						request.setCommand(cfapi::SUBSCRIBE);
					else if (req.compare("UNSUBSCRIBE") == 0)
						request.setCommand(cfapi::UNSUBSCRIBE);
					else if (req.compare("QUERYDEPTH") == 0)
						request.setCommand(cfapi::QUERYDEPTH);
					else if (req.compare("QUERYSNAP") == 0)
						request.setCommand(cfapi::QUERYSNAP);
					else if (req.compare("QUERYWILDCARD") == 0)
						request.setCommand(cfapi::QUERYWILDCARD);
					else if (req.compare("QUERYDEPTHANDSUBSCRIBE") == 0)
						request.setCommand(cfapi::QUERYDEPTHANDSUBSCRIBE);
					else if (req.compare("QUERYSNAPANDSUBSCRIBE") == 0)
						request.setCommand(cfapi::QUERYSNAPANDSUBSCRIBE);
					else if (req.compare("LISTENUMERATION") == 0)
						request.setCommand(cfapi::LISTENUMERATION);
					else if (req.compare("LISTAVAILABLETOKENS") == 0)
						request.setCommand(cfapi::LISTAVAILABLETOKENS);
					else if (req.compare("LISTADMINISTRATIONINFO") == 0)
						request.setCommand(cfapi::LISTADMINISTRATIONINFO);
					else if (req.compare("LISTEXTENDEDEXCHANGEINFO") == 0)
						request.setCommand(cfapi::LISTEXTENDEDEXCHANGEINFO);
					else if (req.compare("SELECTUSERFILTERTOKENS") == 0)
						request.setCommand(cfapi::SELECTUSERFILTERTOKENS);
					else if (req.compare("LISTSUBSCRIBEDSYMBOLS") == 0)
						request.setCommand(cfapi::LISTSUBSCRIBEDSYMBOLS);
 					else if (req.compare("QUERYXREF") == 0)
						request.setCommand(cfapi::QUERYXREF);
 					else if (req.compare("LISTUSERPERMISSION") == 0)
						request.setCommand(cfapi::LISTUSERPERMISSION);
					else if (req.compare("SETCONFLATIONINTERVAL") == 0)
						request.setCommand(cfapi::SETCONFLATIONINTERVAL);
					else if (req.compare("SUBSCRIBEWILDCARD") == 0)
						request.setCommand(cfapi::SUBSCRIBEWILDCARD);
					else if (req.compare("QUERYSNAPANDSUBSCRIBEWILDCARD") == 0)
						request.setCommand(cfapi::QUERYSNAPANDSUBSCRIBEWILDCARD);
					else if (req.compare("UNSUBSCRIBEWILDCARD") == 0)
						request.setCommand(cfapi::UNSUBSCRIBEWILDCARD);
					else if (req.compare("ADDALTERNATEINDEX") == 0)
						request.setCommand(cfapi::ADDALTERNATEINDEX);
					else if (req.compare("GETSOURCEPORTDETAILS") == 0)
						request.setCommand(cfapi::GETSOURCEPORTDETAILS);
					else if (req.compare("SETL2CONFLATIONINTERVAL") == 0)
						request.setCommand(cfapi::SETL2CONFLATIONINTERVAL);
					else
					{	
						printf("Invalid command: %s\n", val.c_str());
						badMsg = true;
					}
					break;
				case 1:
					request.add(cfapi::ENUM_SRC_ID, val.c_str());
					break;
				case 2:
					request.add(cfapi::SYMBOL_TICKER, val.c_str());
					break;
				case 3:
					request.add(cfapi::CUSIP, val.c_str());
					break;
				case 4:
					request.add(cfapi::SEDOL, val.c_str());
					break;
				case 5:
					request.add(cfapi::ISIN, val.c_str());
					break;
				case 6:
					request.add(cfapi::ENUM_SRC_UNDERLYING_ID, val.c_str());
					break;
				case 7:
					request.add(cfapi::SYMBOL_UNDERLYING_TICKER, val.c_str());
					break;
				case 8:
					request.add(cfapi::CONFLATION, val.c_str());
					break;
				case 9:
					request.add(cfapi::CTF_TOKEN_NUM, val.c_str());
					break;
				case 10:
					request.add(cfapi::SYMBOL_BLOOMBERG_TICKER, val.c_str());
					break;
				case 11:
					request.add(cfapi::PRODUCT_ROOT, val.c_str());
					break;
                case 12:
                    request.add(cfapi::CTF_TOKEN_NAME, val.c_str());
                    break;
				case 13:
					request.add(cfapi::SYMBOL_ESIGNAL_TICKER, val.c_str());
					break;
				case 14:
					request.add(cfapi::DEPTH_TYPE, val.c_str());
					break;
				case 15:
					request.add(cfapi::CONFLATION_INTERVAL, val.c_str());
					break;
				case 16:
					request.add(cfapi::CTF_FILTER_ID, val.c_str());
					break;
				case 17:
					request.add(cfapi::QUERY_REF_TAG, (int64_t) strtoll(val.c_str(), NULL, 10));
					break;
				case -1:	//Allow using specific token number
					ctfid = atoi(cmd.c_str());
					if (ctfid == 0)
					{
						badMsg=true;
						printf("Invalid token in the request: %s\n", cmd.c_str());
					}
					else
						request.add(ctfid, val.c_str());
					break;
				default:	//save as parm 
					//request.add(toknum, val);  //ignore
					break;
				}
				if (badMsg)
					break;
			}

			//API send request -- if ok
			if (!badMsg)
			{
				reqHandle = request.generateTag();	//pre-generate unique tag
				reqMapMutex.lock();
				reqMap[reqHandle] = origbuf;
				reqMapMutex.unlock();
				reqHandle = session.send(request);
				if (reqHandle == 0)
					queueFull = true;
			}
			if (!gProgData.mQuiet)
			{
				if (badMsg)
					printf( "Invalid message not sent:[%s]\n", origbuf.c_str());
				else if (queueFull)
					if (gProgData.mWatchlist)
						printf("Error: watchlist and/or send queue is full, cannot send %s\n", origbuf.c_str());
					else
						printf("Error: send queue is full, cannot send %s\n", origbuf.c_str());
				else
					printf( "Send:[%s]: tag =[%lld]\n", origbuf.c_str(), (long long)reqHandle);
			}
		}
		printf("\n");
		fclose(testdata);
	}

	session.freeRequest(request);	//done with request object

	return NULL;
}

#endif /* DOXYGEN_SHOULD_SKIP_THIS */


